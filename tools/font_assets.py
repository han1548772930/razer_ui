"""Preserve current font outlines while applying CSS @font-face metadata."""
from hashlib import sha256
from fontTools.ttLib import TTFont

# Current settings CSS @font-face declarations, shared by product pages.
FACES = (
    ("Roboto-Light", "Roboto", "Light", 300),
    ("Roboto-LightItalic", "Roboto", "Light Italic", 300),
    ("Roboto-Regular", "Roboto", "Regular", 400),
    ("Roboto-Italic", "Roboto", "Italic", 400),
    ("Roboto-Medium", "Roboto", "Medium", 500),
    ("Roboto-MediumItalic", "Roboto", "Medium Italic", 500),
    ("Roboto-Bold", "Roboto", "Bold", 700),
    ("Roboto-BoldItalic", "Roboto", "Bold Italic", 700),
    ("RazerF5-Thin", "RazerF5", "Thin", 100),
    ("RazerF5-Regular", "RazerF5", "Regular", 400),
    ("RazerF5-RegItalic", "RazerF5", "Italic", 400),
    ("RazerF5-SemiBold", "RazerF5", "SemiBold", 600),
    ("RazerF5-Bold", "RazerF5", "Bold", 700),
    ("RazerF5-BoldItalic", "RazerF5", "Bold Italic", 700),
)


def prepare(root, out):
    records = []
    for filename, family, style, weight in FACES:
        source = root / "local-ui-reverse/source/official/apps.razer.com/synapse/assets/fonts" / (filename + ".woff2")
        target = out / (filename + ".ttf")
        font = TTFont(source, recalcTimestamp=False, recalcBBoxes=False)
        original = {"family": font["name"].getDebugName(1),
                    "subfamily": font["name"].getDebugName(2),
                    "preferred_family": font["name"].getDebugName(16),
                    "preferred_subfamily": font["name"].getDebugName(17),
                    "weight": font["OS/2"].usWeightClass}
        preserved = {tag: font.getTableData(tag) for tag in font.keys()
                     if tag not in {"GlyphOrder", "name", "OS/2", "head"}}
        italic = "Italic" in style
        normalize = (original["family"] != family or original["weight"] != weight
                     or original["subfamily"] != style)
        if normalize:
            names = {1: family, 2: style, 16: family, 17: style}
            for record in list(font["name"].names):
                if record.nameID in names:
                    font["name"].setName(names[record.nameID], record.nameID,
                                         record.platformID, record.platEncID, record.langID)
            for name_id, value in names.items():
                font["name"].setName(value, name_id, 3, 1, 0x409)
            font["OS/2"].usWeightClass = weight
            # CSS style/weight are separate from the outline's legacy label.
            font["OS/2"].fsSelection &= ~((1 << 0) | (1 << 5) | (1 << 6))
            if italic:
                font["OS/2"].fsSelection |= 1 << 0
            if weight >= 700:
                font["OS/2"].fsSelection |= 1 << 5
            elif not italic:
                font["OS/2"].fsSelection |= 1 << 6
            font["head"].macStyle = (font["head"].macStyle & ~3) | (weight >= 700) | (int(italic) << 1)
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
            "css_face": {"family": family, "style": "italic" if italic else "normal", "weight": weight},
            "original_metadata": original,
            "metadata_normalized": normalize,
            "unchanged_tables": {tag: sha256(data).hexdigest() for tag, data in preserved.items()},
        })
    return records
