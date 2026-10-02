"""Prepare bundled resources from the audited local reference; never modifies .ref.
Requires Pillow, fonttools and brotli. Customize geometry comes from original groupList literals.
Run from the repository root after tools/extract-keyboard.cjs.
"""
import base64, hashlib, json, re, shutil
from pathlib import Path
import xml.etree.ElementTree as ET
from PIL import Image
from fontTools.ttLib import TTFont
from fontTools.svgLib.path import parse_path
from fontTools.pens.boundsPen import BoundsPen
from keyboard_geometry import geometry_for
from gamer_room_assets import prepare as prepare_gamer_room_devices

ROOT=Path(__file__).resolve().parent.parent
OUT=ROOT/"assets"/"synapse"
OUT.mkdir(parents=True, exist_ok=True)
records=[]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def record(src,dst,**meta):
    records.append(dict(source=src.relative_to(ROOT).as_posix(),output=dst.relative_to(ROOT).as_posix(),source_sha256=digest(src),sha256=digest(dst),**meta))

# Electron TabUI uses separate SVGs for host controls and their pointer states.
for name in ("minimize", "maximize", "restore", "close", "close-original",
             "close_active_tab", "close_active_tab_hover", "close_pressed",
             "left-arrow", "left-arrow-hover", "left-arrow-active", "left-arrow-disabled",
             "right-arrow", "right-arrow-hover", "right-arrow-active", "right-arrow-disabled"):
    src = ROOT / ".ref/synapse-asar/electron/assets/image/tab" / (name + ".svg")
    dst = OUT / ("host-" + name + ".svg")
    shutil.copyfile(src, dst)
    record(src, dst)

for category in ("MOUSE", "KEYBOARD", "AUDIO"):
    src = ROOT / ".ref/frontend/shared-favicon" / (category + ".svg")
    dst = OUT / ("host-category-" + category.lower() + ".svg")
    shutil.copyfile(src, dst)
    record(src, dst, source_url="https://apps.razer.com/synapse/assets/imgs/favicon/" + category + ".svg")

images={
    "mouse-182.png": (182,"prd-3x.674b18f0.avif"),
    "mouse-182-bottom.png": (182,"litat1-profile-button-3x.99ffa7e1.avif"),
    "keyboard-653.png": (653,"1-3x.8d525695.avif"),
    "keyboard-653-wrist.png": (653,"wrist-without-gap.efc0505f.avif"),
    "keyboard-653-dial.png": (653,"dial-3x-1-2x.a7526df5.avif"),
    "keyboard-653-digital-dial.png": (653,"digital-dial.1e701318.avif"),
    "keyboard-653-wrist-white.png": (653,"wrist-without-gap-white.f54a214f.avif"),
    "keyboard-653-roller.png": (653,"roller-3x.7fe981b9.avif"),
    "keyboard-653-roller-white.png": (653,"roller-white-3x.7863b66d.avif"),
    "headset-777.png": (777,"prd-3x.ede8800a.avif"),
    "stream-777.png": (777,"stream-3x.e6572c2c.avif"),
    "install-chroma.png": (653,"install_chroma.85d4bc96.avif"),
}
for name,(pid,original) in images.items():
    src=ROOT/".ref"/"devices"/str(pid)/"static"/"media"/original
    dst=OUT/name
    with Image.open(src) as im:
        im=im.convert("RGBA")
        im.save(dst,optimize=True)
        record(src,dst,width=im.width,height=im.height,mode=im.mode)

# Dashboard/Gamer Room media are shared frontend assets, not device variants.
# Their exact request names come from 55 CSS / 9388 and the main chunk.
frontend_media = {
    "gr-background.png": "gamer-room-bg-image@3x.ed0ae6ff.avif",
    "gr-bulb.png": "aether-bulb-marketing-image.2048a07b.avif",
    "gr-strip.png": "aether-strip-marketing-image.797da4bf.avif",
    "gr-lamp-pro.png": "aether-lamp-pro-marketing-image.57b787df.avif",
    "gr-app.svg": "app-gamer-room-icon.01c86f10.svg",
    "gr-qr.svg": "qr-code-gamer-room.58677794.svg",
    "gr-add-device.svg": "icon_add_smart_home_device_outline.01f8b64d.svg",
    "gr-mobile.svg": "icon_mobile_download.858c66c9.svg",
    "gr-tutorial-indicator.svg": "indicator.b7ce7af4.svg",
    "iot-key-light.svg": "icon_add_key_light_outline.2bc5f2e7.svg",
    "iot-streaming-app.svg": "app-streaming-icon.6be479a2.svg",
    "iot-key-light-qr.svg": "qr-code-key-light.f91e9fbe.svg",
    "iot-refresh.svg": "icon_refresh-2.de191fed.svg",
    "iot-refresh-green.svg": "icon_refresh_3.4e3a6824.svg",
    "iot-new-help.svg": "icon_new_iot_device_tooltip.990363a0.svg",
    "iot-see-password.svg": "see_password.123e5e09.svg",
    "iot-not-found.svg": "icon_not_found.98525e61.svg",
    "iot-wifi-error.svg": "icon_wifi_error.a355ac62.svg",
    "iot-success.svg": "icon_success_gray.14655ce6.svg",
    "iot-identify.svg": "icon_identify_devices.0e6bf7b8.svg",
    "iot-wifi-device.svg": "icon_wifi_device.19a8e226.svg",
    "iot-wifi-add.svg": "icon_wifi_add.5fd71806.svg",
    "dashboard-store.png": "services_store_placeholder.9f5d3c9a.avif",
    "dashboard-support.png": "services_support_placeholder.62c592fb.avif",
    "dashboard-community.png": "services_community_placeholder.50541baa.avif",
    "dashboard-gold.png": "service_gold_and_silver.12f2d9c9.avif",
    "dashboard-iot.svg": "add_wifi_device.60d9fd5b.svg",
    "dashboard-tour.svg": "logo_tour.c14123f7.svg",
    "account-guest.svg": "guest.00470cfc.svg",
    "gr-device-spinner.svg": "spinner.ef2d0235.svg",
    "module-macro.svg": "logo_macro.9a6b554a.svg",
    "module-alexa.svg": "logo_alexa.b9b62050.svg",
    "module-linked-games.svg": "logo_linked_games.b9bd8037.svg",
    "module-feedback.svg": "logo_feedback.0ee7f6c6.svg",
    "module-armory.svg": "logo_armory_workshop.9cfc256a.svg",
    "module-macro.png": "macro.4afae13e.avif",
    "module-alexa.png": "alexa.110b43c3.avif",
}
for name, original in frontend_media.items():
    src = ROOT / ".ref/frontend/static/media" / original
    dst = OUT / name
    source_url = "https://apps.razer.com/synapse/dashboard/static/media/" + original
    if dst.suffix == ".png":
        with Image.open(src) as im:
            im = im.convert("RGBA")
            im.save(dst, optimize=True)
            record(src, dst, width=im.width, height=im.height, mode=im.mode,
                   source_url=source_url)
    else:
        shutil.copyfile(src, dst)
        record(src, dst, source_url=source_url)

prepare_gamer_room_devices(ROOT, OUT, record)
for category in ("KEY_LIGHT", "BULB", "LAMP", "STRIP"):
    name = f"IOT_{category}.svg"
    src = ROOT / ".ref/frontend/shared-favicon" / name
    dst = OUT / ("iot-category-" + category.lower().replace("_", "-") + ".svg")
    shutil.copyfile(src, dst)
    record(src, dst, source_url="https://apps.razer.com/synapse/assets/imgs/favicon/" + name)

src = OUT / "iot-key-light-product-source.avif"
dst = OUT / "iot-key-light-product.png"
with Image.open(src) as im:
    im = im.convert("RGBA")
    im.save(dst, optimize=True)
    record(src, dst, width=im.width, height=im.height, mode=im.mode,
           source_url="https://apps.razer.com/synapse/products/780/ui/780_0/PluginImages/780_0_0_dashboard1x.avif")

# Header compatibility dialog's inline warning triangle (not Settings' circle).
src = ROOT / ".ref/frontend/static/js/App.eb32d7cd.chunk.js"
header_source = src.read_text(encoding="utf-8")
header_start = header_source.index("27875:")
header_end = header_source.index("},28648:", header_start)
warning = re.search(r'fill:"(#fd8611)",d:"([^"]+)"', header_source[header_start:header_end])
assert warning, "Missing compatibility warning in module 27875"
dst = OUT / "header-compatibility-warning.svg"
dst.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="27" viewBox="0 0 20 20"><path fill="' + warning[1] + '" d="' + warning[2] + '"/></svg>\n', encoding="utf-8")
record(src, dst, module=27875, viewBox=[0, 0, 20, 20], purpose="Compatibility mode warning")

# Ve's new-device help uses an inline 285x145 MAC-address illustration.
src = ROOT / ".ref/frontend/static/js/IotPopupRoot.290be417.chunk.js"
iot_source = src.read_text(encoding="utf-8")
mac_image = re.search(r'src:"data:image/png;base64,([A-Za-z0-9+/=]+)"', iot_source)
assert mac_image, "Missing new-device MAC illustration"
dst = OUT / "iot-new-device-mac.png"
dst.write_bytes(base64.b64decode(mac_image[1], validate=True))
with Image.open(dst) as im:
    assert im.size == (285, 145), "Unexpected MAC illustration size"
    record(src, dst, width=im.width, height=im.height, mode=im.mode,
           source_offset=mac_image.start(), purpose="New-device MAC-address help")

# Category icons used by 6505's device/install/firmware rows are generated by
# App module 96689's renderer c, not the similarly named key-mapping SVGs.
# Keep extraction within that module and use its final `i` transform table.
src = ROOT / ".ref/frontend/static/js/App.eb32d7cd.chunk.js"
category_source = src.read_text(encoding="utf-8")
category_start = category_source.index("96689:")
category_end = category_source.index("},96776:", category_start)
category_module = category_source[category_start:category_end]
category_transforms = re.search(r'\bi=\{(.*?)\},o=\(', category_module)
assert category_transforms, "Missing final category transform map in module 96689"
for category in ("KEYBOARD", "MOUSE"):
    path = re.search(r'\{name:"' + category + r'",d:"([^"]*)"\}', category_module)
    transform = re.search(category + r':\{transformX:(-?\d+),transformY:(-?\d+)\}', category_transforms[1])
    assert path and transform, f"Missing source category icon {category}"
    x, y = transform.groups()
    dst = OUT / f"category-{category.lower()}.svg"
    dst.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" fill="#ccc" viewBox="0 0 20 20"><g transform="translate(0 0)"><path d="'
                   + path[1] + '" transform="translate(' + x + ' ' + y + ')"/></g></svg>\n', encoding="utf-8")
    record(src, dst, module=96689, source_offset=category_start + path.start(),
           category=category, transform=f"translate({x} {y})", viewBox=[0, 0, 20, 20],
           purpose="Original 40px device category icon from renderer c and final transform map i")

for name, original in {
    "settings-synapse-logo.svg": "razer_synapse_4-1.56e5c81a.svg",
    "settings-wdl.svg": "wdl_large_icon.e610fd8c.svg",
    "settings-warning.svg": "exclamation_icon.53d6d40b.svg",
    "header-offline.svg": "cloud-yellow.3957bb55.svg",
    "header-update.svg": "icon_download.1e6d735a.svg",
}.items():
    src = ROOT / ".ref/settings/static/media" / original
    dst = OUT / name
    shutil.copyfile(src, dst)
    record(src, dst, source_url="https://apps.razer.com/synapse/settings/static/media/" + original)

src = ROOT / ".ref/release-patch-note/static/media/open_in_new_icon.c029c0d0.svg"
dst = OUT / "release-notes-external.svg"
shutil.copyfile(src, dst)
record(src, dst, source_url="https://apps.razer.com/release-patch-note/static/media/open_in_new_icon.c029c0d0.svg")

# Settings Fs renders inline SVGs, whose shared .ellipse/.social rules are
# emitted by Facebook and YouTube. The separate media copies have different
# (and in Instagram's case commented-out) CSS. Resolve the actual inline
# cascade into static attributes; the native image renderer has no :hover.
src = ROOT / ".ref/settings/static/js/720.1e5d1c8f.chunk.js"
social_source = src.read_text(encoding="utf-8")
social_icons = {
    "facebook": "Like us on Facebook, Facebook",
    "instagram": "Follow us on Instagram, instagram",
    "twitter": "Follow us on X, x",
    "youtube": "Subscribe to our YouTube channel, Youtube",
    "tiktok": "Follow us on TikTok, tiktok",
    "twitch": "Subscribe to our Twitch channel, twitch",
    "discord": "Join our Discord, Discord",
    "insider": "Razer insider",
}
social_components = {}
for name, label in social_icons.items():
    start = social_source.index('"aria-label":"' + label + '"')
    end = re.search(r'\},\w+=\(0,o.forwardRef\)', social_source[start:])
    assert end, f"Missing inline social component end: {name}"
    social_components[name] = (start, social_source[start:start + end.start()])

def inline_social_styles(component):
    return [json.loads('"' + match[1] + '"') for match in re.finditer(
        r'o\.createElement\("style",\{[^}]*\},"((?:\\.|[^"\\])*)"\)', component)]

shared_social_css = "\n".join(inline_social_styles(social_components[name][1])[0]
                              for name in ("facebook", "youtube"))
assert all(not inline_social_styles(social_components[name][1])
           for name in ("instagram", "twitter", "tiktok", "twitch", "discord")), "Re-audit changed inline SVG style cascade"

def social_style_for(stylesheet, class_name, hovered):
    result = {}
    for selector, declarations in re.findall(r'([^{}]+)\{([^}]*)\}', stylesheet):
        selector = re.sub(r'\s+', '', selector)
        if ':hover' in selector and not hovered:
            continue
        selector = selector.replace(':hover', '')
        if selector not in ('.' + class_name, '.ellipse~.social' if class_name == 'social' else ''):
            continue
        for key, value in re.findall(r'([\w-]+)\s*:\s*([^;]+)', declarations):
            if key not in ('transition', 'pointer-events'):
                result[key] = value.strip()
    return result

for name, (start, component) in social_components.items():
    stylesheet = ('\n'.join(inline_social_styles(component))
                  if name == 'insider' else shared_social_css)
    width, height = (270, 50) if name == 'insider' else (28, 28)
    shapes = list(re.finditer(r'o\.createElement\("(circle|path|rect)",\{([^}]*)\}\)', component))
    assert shapes and ('transform:' not in component if name == 'insider' else True)
    for hovered in (False, True):
        root = ET.Element('svg', {'xmlns': 'http://www.w3.org/2000/svg',
                                'width': str(width), 'height': str(height),
                                'viewBox': f'0 0 {width} {height}'})
        for shape in shapes:
            attributes = {}
            for key, value in re.findall(r'(\w+):("(?:\\.|[^"\\])*"|-?[\d.]+)', shape[2]):
                attributes['class' if key == 'className' else key] = json.loads(value) if value.startswith('"') else value
            attributes.update(social_style_for(stylesheet, attributes.get('class', ''), hovered))
            ET.SubElement(root, shape[1], attributes)
        dst = OUT / ('settings-social-' + name + ('-hover' if hovered else '') + '.svg')
        ET.ElementTree(root).write(dst, encoding='utf-8', xml_declaration=True)
        record(src, dst, source_offset=start, inline_component=name,
               width=width, height=height, state='hover' if hovered else 'normal',
               transform='Extract inline SVG literal shapes; resolve Fs shared Facebook/YouTube CSS or Insider CSS to static attributes; omit transitions')

src = OUT / "gr-action-arrow-source.svg"
dst = OUT / "gr-action-arrow.svg"
shutil.copyfile(src, dst)
record(src, dst, source_kind="downloaded_static_asset",
       source_url="https://apps.razer.com/synapse/dashboard/static/media/icon_arrow_short_right.5a823bac.svg")

# Native SVG rendering does not run SMIL. The original ellipses have no rx/ry
# until animation runs, so preserve the original 0.25 keyframe explicitly.
src = OUT / "gr-hotspot-source.svg"
hotspot = ET.parse(src).getroot()
ns = "{http://www.w3.org/2000/svg}"
for animation in hotspot.findall(f"{ns}defs/{ns}animate"):
    target_id = animation.get("{http://www.w3.org/1999/xlink}href", "").removeprefix("#")
    target = hotspot.find(f".//*[@id='{target_id}']")
    if target is not None and animation.get("values"):
        times = animation.attrib["keyTimes"].split(";")
        target.set(animation.attrib["attributeName"],
                   animation.attrib["values"].split(";")[times.index("0.25")])
hotspot.remove(hotspot.find(f"{ns}defs"))
dst = OUT / "gr-hotspot.svg"
ET.ElementTree(hotspot).write(dst, encoding="utf-8", xml_declaration=True)
record(src, dst,
       source_url="https://apps.razer.com/synapse/dashboard/static/media/gamer_room_hotspot_animation.53dd5566.svg",
       source_kind="downloaded_static_asset",
       transform="freeze original SMIL at keyTime 0.25 (1/3 second); omit animation nodes")

# Module 71610 exports a literal PNG data URL. Decode the bytes without
# evaluating any source module or changing the original image.
src = ROOT / ".ref/frontend/static/js/9388.2bec5db3.chunk.js"
lamp = re.search(r'71610:\w+=>\{"use strict";\w+\.exports="data:image/png;base64,([A-Za-z0-9+/=]+)"',
                 src.read_text(encoding="utf-8"))
assert lamp, "Missing Gamer Room Aether Lamp module 71610"
dst = OUT / "gr-lamp.png"
dst.write_bytes(base64.b64decode(lamp[1], validate=True))
with Image.open(dst) as im:
    record(src, dst, width=im.width, height=im.height, mode=im.mode,
           module=71610, purpose="Gamer Room Aether Lamp literal PNG data URL")

# Statically resolve Webpack's request maps and exact media-export modules.
# Do not execute downloaded JS or guess a variant from a similar filename.
module_export = re.compile(
    r'(?<![\w])(?P<id>\d+):\([^)]*\)=>\{(?:"use strict";)?'
    r'\w+\.exports=\w+\.p\+"(?P<path>static/media/[^"]+)";?\}'
)
product_requests = []
product_assets = []
outputs = {entry["source"]: entry for entry in records}
for pid in (182, 653, 777):
    scripts = sorted((ROOT / f".ref/devices/{pid}/static/js").glob("*.js"))
    main = next(path for path in scripts if path.name.startswith("main."))
    main_source = main.read_text(encoding="utf-8")
    modules = {}
    for script in scripts:
        for match in module_export.finditer(script.read_text(encoding="utf-8")):
            value = (match["path"], script)
            module_id = int(match["id"])
            assert module_id not in modules or modules[module_id][0] == value[0], module_id
            modules[module_id] = value
    requests = re.findall(
        r'"(\./' + str(pid) + r'_(\d+)/img_prods/([^"\\]+))":\[(\d+),[^\]]+\]',
        main_source,
    )
    seen = set()
    for request, edition, name, module_id in sorted(set(requests)):
        media, script = modules[int(module_id)]
        src = ROOT / f".ref/devices/{pid}" / media
        source_key = src.relative_to(ROOT).as_posix()
        if source_key not in outputs:
            dst = OUT / f"product-{pid}-{src.stem}.png"
            with Image.open(src) as im:
                im = im.convert("RGBA")
                im.save(dst, optimize=True)
                record(src, dst, width=im.width, height=im.height, mode=im.mode)
            outputs[source_key] = records[-1]
        entry = outputs[source_key]
        reference = dict(request=request, module=int(module_id),
                         context=main.relative_to(ROOT).as_posix(),
                         context_sha256=digest(main),
                         module_file=script.relative_to(ROOT).as_posix(),
                         module_sha256=digest(script))
        entry.setdefault("webpack_requests", []).append(reference)
        product_requests.append(dict(product_id=pid, edition_id=int(edition),
                                     **reference, source=source_key, output=entry["output"]))
        if name == "prd-3x.png" and pid in (182, 777):
            layout, purpose = 0, "Product"
        elif name == "litat1-profile-button-3x.png" and pid == 182:
            layout, purpose = 0, "MouseBottom"
        elif pid == 653 and re.fullmatch(r"\d+-3x\.png", name):
            layout, purpose = int(name.split("-")[0]), "Product"
        else:
            continue
        identity = (pid, int(edition), layout, purpose)
        assert identity not in seen, identity
        seen.add(identity)
        product_assets.append((*identity, "synapse/" + Path(entry["output"]).name))

# km selects the white wrist/roller only for edition 130. These parts have no
# layout-specific geometry; their visibility still comes from device state.
for edition in (0, 128, 129, 130):
    product_assets.append((653, edition, 0, "KeyboardWrist",
        "synapse/keyboard-653-wrist-white.png" if edition == 130 else "synapse/keyboard-653-wrist.png"))
    product_assets.append((653, edition, 0, "KeyboardDial",
        "synapse/keyboard-653-roller-white.png" if edition == 130 else "synapse/keyboard-653-roller.png"))
product_assets.append((777, 0, 0, "Streamer", "synapse/stream-777.png"))
(OUT / "product-image-map.json").write_text(json.dumps(dict(
    version=1, requests=product_requests,
    resolved=[dict(product_id=pid, edition_id=edition, layout_id=layout, part=part, asset=asset)
              for pid, edition, layout, part, asset in sorted(product_assets)]), indent=2), encoding="utf-8")
(OUT / "product-images.rs").write_text(
    "// Generated from original Webpack contexts by tools/prepare-resources.py.\n&[\n"
    + "".join(f'    ({pid}, {edition}, {layout}, DeviceImage::{part}, "{asset}"),\n'
              for pid, edition, layout, part, asset in sorted(product_assets))
    + "]\n", encoding="utf-8")
# Dashboard and pairing use PluginImages, not Customize's img_prods. These
# downloaded AVIF sources follow the original URL rule in 4130. Do not fetch
# anything while preparing resources, and deduplicate identical image bytes.
dashboard_requests = []
dashboard_outputs = {}
dashboard_variants = sorted({(pid, edition, layout)
                            for pid, edition, layout, part, _ in product_assets
                            if part == "Product"})
for pid, edition, layout in dashboard_variants:
    src = OUT / f"dashboard-{pid}-{edition}-{layout}-source.avif"
    source_url = (f"https://apps.razer.com/synapse/products/{pid}/ui/{pid}_{edition}/PluginImages/"
                  f"{pid}_{edition}_{layout}_dashboard3x.avif")
    source_hash = digest(src)
    if source_hash not in dashboard_outputs:
        name = "dashboard-182.png" if (pid, edition, layout) == (182, 0, 0) else f"dashboard-{pid}-{edition}-{layout}.png"
        dst = OUT / name
        with Image.open(src) as im:
            im = im.convert("RGBA")
            im.save(dst, optimize=True)
            record(src, dst, width=im.width, height=im.height, mode=im.mode,
                   source_url=source_url, source_kind="downloaded_static_asset",
                   purpose="Original PluginImages Dashboard/pairing card artwork")
        dashboard_outputs[source_hash] = dst
    dst = dashboard_outputs[source_hash]
    dashboard_requests.append(dict(product_id=pid, edition_id=edition, layout_id=layout,
                                   source=src.relative_to(ROOT).as_posix(), source_sha256=source_hash,
                                   source_url=source_url, asset="synapse/" + dst.name,
                                   output=dst.relative_to(ROOT).as_posix()))
(OUT / "dashboard-image-map.json").write_text(json.dumps(dict(
    version=1, url_rule_source=".ref/frontend/static/js/4130.155387bf.chunk.js",
    requests=dashboard_requests), indent=2), encoding="utf-8")
(OUT / "dashboard-images.rs").write_text(
    "// Generated from downloaded PluginImages sources; never substitute Customize artwork.\n&[\n"
    + "".join(f'    ({r["product_id"]}, {r["edition_id"]}, {r["layout_id"]}, "{r["asset"]}"),\n'
              for r in dashboard_requests)
    + "]\n", encoding="utf-8")
# Profile Migration is a separate app opened by the Settings Ta component.
migration_media = ROOT / ".ref/profile-migration/static/media"
for name, original in {
    "migration-date.svg": "common-date.8cb15e00.svg",
    "migration-file-warning.svg": "warning.2c334b3a.svg",
    "migration-game-warning.svg": "linkedGameWarning.e79a4da8.svg",
    "migration-macro-icon.svg": "macro-icon.03bcda37.svg",
    "migration-macro-warning.svg": "synapse_no_macro.0349793c.svg",
    "migration-tick.svg": "tick-solid.c2eb725a.svg",
    "migration-unused.svg": "disable.4679810a.svg",
}.items():
    src, dst = migration_media / original, OUT / name
    shutil.copyfile(src, dst)
    record(src, dst)
src = migration_media / "macro-module.7be6db02.avif"
dst = OUT / "migration-macro.png"
with Image.open(src) as image:
    image = image.convert("RGBA")
    image.save(dst, optimize=True)
    record(src, dst, width=image.width, height=image.height, mode=image.mode)
src = ROOT / ".ref/profile-migration/static/js/main.512f18b6.js"
chroma_png = re.search(r'Dg=\{"chroma-app":"data:image/png;base64,([^"]+)"', src.read_text(encoding="utf-8"))
assert chroma_png, "Missing original migration Chroma artwork"
dst = OUT / "migration-chroma.png"
dst.write_bytes(base64.b64decode(chroma_png.group(1)))
with Image.open(dst) as image:
    record(src, dst, width=image.width, height=image.height, mode=image.mode,
           transform="decode original Dg chroma-app PNG data URL")

vectors = {
    "onboard-info.svg": (653, "info_outline.b491abc9.svg"),
    "onboard-transfer-warning.svg": (653, "icon_warning.6c0cd78b.svg"),
    "onboard-overlay-error.svg": (653, "icon_obm_overlay_error.36277c8e.svg"),
    "onboard-overlay-sync.svg": (653, "icon_obm_overlay_sync.6c1ca10a.svg"),
    "dial-add.svg": (653, "icon_add_default.9e9b3892.svg"),
    "dial-add-hover.svg": (653, "icon_add_hover.9c2128db.svg"),
    "dial-add-disabled.svg": (653, "icon_addnotgrey.f73db7b6.svg"),
    "dial-more.svg": (653, "icon_more.fb688d78.svg"),
    "dial-more-hover.svg": (653, "icon_more_hover.deac19e5.svg"),
    "plus.svg": (653, "plus.a386742c.svg"),
    "onboard-lock.svg": (653, "icon_lock.8f4a479b.svg"),
    "onboard-error.svg": (653, "icon_obm_error.7fee0731.svg"),
    "onboard-macro.svg": (653, "icon_macro.1d733729.svg"),
    "onboard-no-space.svg": (653, "icon_obm_na.5e6daed8.svg"),
    "onboard-help.svg": (653, "tooltip_questionmark.96138d2f.svg"),
    "onboard-warning.svg": (653, "warning.ad3f47f8.svg"),
    "color-picker.svg": (653, "color_picker.b2c4481d.svg"),
    "color-picker-hover.svg": (653, "color_picker_hover.c1c1c043.svg"),
    "dial-delete.svg": (653, "icon_delete.de9b7746.svg"),
    "dial-delete-hover.svg": (653, "icon_delete_hover.1245ec4b.svg"),
    "dial-reset.svg": (653, "icon_refresh_default.93489aa0.svg"),
    "dial-reset-hover.svg": (653, "icon_refresh_hover.18c8e3a0.svg"),
    "mapping-close.svg": (182, "icon_close.55fe41f1.svg"),
    "mapping-brightness-active.svg": (182, "icon_config_brightness_a.570b75be.svg"),
    "mapping-default-active.svg": (182, "icon_config_default_a.0946ac79.svg"),
    "mapping-keyboard-active.svg": (182, "icon_config_keyboard_a.7051c99b.svg"),
    "mapping-mouse-active.svg": (182, "icon_config_mouse_a.e80d52da.svg"),
    "mapping-sensitivity-active.svg": (182, "icon_config_mouse_sensitivity_a.7f79e597.svg"),
    "mapping-macro-active.svg": (182, "icon_config_macro_a.27e913c3.svg"),
    "mapping-interdevice-active.svg": (182, "icon_config_interdevice_a.87c34f05.svg"),
    "mapping-profile-active.svg": (182, "icon_config_switch_device_profile_a.942d360a.svg"),
    "mapping-hypershift-active.svg": (182, "icon_config_hypershift_a.33a591f6.svg"),
    "mapping-launch-active.svg": (182, "icon_config_launch_a.52c03b9d.svg"),
    "mapping-multimedia-active.svg": (182, "icon_config_multimedia_a.b2da1c02.svg"),
    "mapping-windows-active.svg": (182, "icon_config_windows_shortcut_a.3751588d.svg"),
    "mapping-text-active.svg": (182, "icon_config_text_a.a7d11e49.svg"),
    "mapping-disable-active.svg": (182, "icon_config_disable_a.6b71a2b4.svg"),
    "mapping-brightness.svg": (182, "icon_config_brightness.7a95c958.svg"),
    "mapping-default.svg": (182, "icon_config_default.46d77571.svg"),
    "mapping-keyboard.svg": (182, "icon_config_keyboard.08cddd8c.svg"),
    "mapping-mouse.svg": (182, "icon_config_mouse.d7500793.svg"),
    "mapping-sensitivity.svg": (182, "icon_config_mouse_sensitivity.cd4e347f.svg"),
    "mapping-macro.svg": (182, "icon_config_macro.243cd8e5.svg"),
    "mapping-interdevice.svg": (182, "icon_config_interdevice.9b1fc6e9.svg"),
    "mapping-profile.svg": (182, "icon_config_switch_device_profile.88b9f2aa.svg"),
    "mapping-hypershift.svg": (182, "icon_config_hypershift.6c83f48a.svg"),
    "mapping-launch.svg": (182, "icon_config_launch.3b800ccf.svg"),
    "mapping-multimedia.svg": (182, "icon_config_multimedia.800d8c90.svg"),
    "mapping-windows.svg": (182, "icon_config_windows_shortcut.580f7d02.svg"),
    "mapping-text.svg": (182, "icon_config_text.3259fd59.svg"),
    "mapping-disable.svg": (182, "icon_config_disable.52df003a.svg"),
    "external-link.svg": (182, "icon_external_link.48227e72.svg"),
    "drawer-close.svg": (182, "icon_closepanel.86903958.svg"),
    "stepper-up.svg": (653, "stepper_up.dcb04520.svg"),
    "stepper-down.svg": (653, "stepper_down.349f755c.svg"),
    "profile-more.svg": (182, "icon_more.fb688d78.svg"),
    "eq-reset.svg": (777, "eq_reset.e0c3c09c.svg"),
    "eq-reset-hover.svg": (777, "eq_reset_hover.186df33c.svg"),
    "eq-reset-active.svg": (777, "eq_reset_active.37c570d3.svg"),
    "keyboard-653-dial-mapping.svg": (653, "dial-mapping.36cf0d1f.svg"),
    "chroma-studio.svg": (653, "chroma_studio.55db6875.svg"),
    "history-back.svg": (182, "icon_arrow_left_thin.e6d37c55.svg"),
    "history-forward.svg": (182, "icon_arrow_right_thin.bef8ca32.svg"),
    "settings.svg": (182, "icon_settings-2.07e96d4c.svg"),
    "profile.svg": (182, "profile-default.f608d82c.svg"),
    "profile-obm.svg": (653, "icon_obm.888208d6.svg"),
    "synapse.svg": (182, "logo_synapse.bc241e5e.svg"),
    "drawer.svg": (182, "icon_sidepanel.e53fef93.svg"),
    "drawer-active.svg": (182, "icon_sidepanel_a.90d67a6e.svg"),
    "calibration-close.svg": (182, "icon_close_white.8ab462b8.svg"),
    "calibration-close-active.svg": (182, "icon_close_green.45f61360.svg"),
    "windows.svg": (182, "windows_logo.8fb1e7e2.svg"),
    "windows-11.svg": (182, "common-windows-11.d477cadb.svg"),
    "expand.svg": (182, "icon_expand.55a47b0c.svg"),
    "expand-hover.svg": (182, "icon_expand_l.3b8f6b34.svg"),
    "dashboard-drag.svg": (182, "icon_draggable_large.e4bda42a.svg"),
    "dpi-draggable.svg": (182, "icon_draggable_large.e4bda42a.svg"),
    "dashboard-add.svg": (182, "icon_add.c95a8d74.svg"),
    "dashboard-add-hover.svg": (182, "icon_add_w.0fc3f789.svg"),
    "sensitivity-xy.svg": (182, "icon_sensitivity_xy.b9eb5286.svg"),
    "sensitivity-xy-active.svg": (182, "icon_sensitivity_xy_active.39d46352.svg"),
    "sensitivity-xy-disabled.svg": (182, "icon_sensitivity_xy_disabled.3a6ee34e.svg"),
    "stage-1.svg": (182, "red.99a70e7d.svg"),
    "stage-2.svg": (182, "green.9abd2946.svg"),
    "stage-3.svg": (182, "blue.c01cdc30.svg"),
    "stage-4.svg": (182, "cyan.02647853.svg"),
    "stage-5.svg": (182, "yellow.a30299c6.svg"),
    "chroma-sync.svg": (653, "chroma_sync_v3_static.c8ddf315.svg"),
    "direction-left.svg": (653, "icon_direction_left_999.3397283a.svg"),
    "direction-left-active.svg": (653, "icon_direction_left.cc325d65.svg"),
    "direction-right.svg": (653, "icon_direction_right_999.8c251efa.svg"),
    "direction-right-active.svg": (653, "icon_direction_right.70dccfc5.svg"),
    "direction-cw.svg": (653, "icon_direction_cw.075c1730.svg"),
    "direction-cw-active.svg": (653, "icon_direction_cw_1.0d8f9301.svg"),
    "direction-ccw.svg": (653, "icon_direction_ccw.e34b12f6.svg"),
    "direction-ccw-active.svg": (653, "icon_direction_ccw_1.3dffaadc.svg"),
    "direction-out.svg": (653, "icon_direction_outward_999.4cd91b14.svg"),
    "direction-out-active.svg": (653, "icon_direction_outward.7c388a12.svg"),
    "direction-in.svg": (653, "icon_direction_inward_999.29221403.svg"),
    "direction-in-active.svg": (653, "icon_direction_inward.29c0ed8a.svg"),
    "palette-none.svg": (653, "disable_palette.03c60895.svg"),
}
for name, (pid, original) in vectors.items():
    src = ROOT / ".ref/devices" / str(pid) / "static/media" / original
    dst = OUT / name
    shutil.copyfile(src, dst)
    record(src, dst)
# Original Chroma category icon is a CSS data URL, not a separate SVG.
src = ROOT / ".ref/devices/182/static/css/main.48c20423.css"
css_text = src.read_text(encoding="utf-8")
for state, suffix in (("", ""), (".open", "-active")):
    selector = ".action-wrapper.SWITCH_LIGHTING" + state + " .head:before"
    chroma_icon = re.search(re.escape(selector) + r"\{background-image:url\(data:image/png;base64,([^)]*)", css_text)
    assert chroma_icon, "Missing original SWITCH_LIGHTING category icon"
    dst = OUT / f"mapping-lighting{suffix}.png"
    dst.write_bytes(base64.b64decode(chroma_icon.group(1)))
    with Image.open(dst) as im:
        record(src, dst, width=im.width, height=im.height, mode=im.mode,
               purpose="SWITCH_LIGHTING category icon from CSS data URL", selector=selector)
# Help uses React inline SVGs, not the ordinary 10px dropdown chevron.
src = ROOT / ".ref/devices/182/static/js/main.db20a7c4.js"
help_source = src.read_text(encoding="utf-8")
for name, symbol in [("help-less.svg", "Ed"), ("help-more.svg", "qR")]:
    match = re.search(r'\b' + symbol + r'=\([^)]*\)=>.{0,4000}?d:"([^"]+)"', help_source)
    assert match, f"Missing original inline help SVG: {symbol}"
    dst = OUT / name
    dst.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20" fill="currentColor"><path d="' + match[1] + '"/></svg>\n', encoding="utf-8")
    record(src, dst, inline_symbol=symbol)

# Chromium selects these sprite viewports with #default/#hover/#active.
# Native image decoders do not select SVG fragments, so set the root viewport.
ET.register_namespace("", "http://www.w3.org/2000/svg")
ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
src = ROOT / ".ref/devices/182/static/media/icon_help.377359c3.svg"
for state in ("default", "hover", "active"):
    root = ET.parse(src).getroot()
    view = root.find(f"{{http://www.w3.org/2000/svg}}view[@id='{state}']")
    root.set("viewBox", view.attrib["viewBox"])
    root.set("width", "26")
    root.set("height", "26")
    dst = OUT / f"help-{state}.svg"
    ET.ElementTree(root).write(dst, encoding="utf-8", xml_declaration=True)
    record(src, dst, fragment=state)
for name in ["Roboto-Regular","Roboto-Medium","Roboto-Bold","RazerF5-Regular"]:
    src=ROOT/".ref/synapse-asar/electron/assets/fonts"/(name+".woff2")
    font=TTFont(src, recalcTimestamp=False); font.flavor=None
    dst=OUT/(name+".ttf");font.save(dst);record(src,dst)
def keyboard_keys(groups):
    keys=[]
    for group in groups:
        for key in group["group"]["buttonList"]:
            if "d" in key:
                pen=BoundsPen(None);parse_path(key["d"],pen)
                x0,y0,x1,y1=pen.bounds
                bounds=[x0,y0,x1-x0,y1-y0]
            elif "r" in key:
                bounds=[key["cx"]-key["r"],key["cy"]-key["r"],key["r"]*2,key["r"]*2]
            elif "x" in key:
                bounds=[key["x"],key["y"],key["width"],key["height"]]
            else: continue
            keys.append(dict(id=str(key["inputID"]),label=str(key["counter"]),bounds=bounds,
                             enabled=key.get("isEnabled",True),functions=key.get("functionList",[]),
                             button_key=key["buttonKey"],default=key.get("defaultValue",""),
                             path=key.get("d"),children=key.get("childrenKey",[]),
                             geometry=geometry_for(key)))
    return keys

layout_sources = json.loads((OUT / "keyboard-653-customize-layouts.json").read_text(encoding="utf-8"))
layouts = []
for layout in layout_sources["layouts"]:
    groups = json.loads((ROOT / layout["output"]).read_text(encoding="utf-8"))
    keys = keyboard_keys(groups)
    assert len({key["id"] for key in keys}) == len(keys), layout["layout_id"]
    layouts.append(dict(layout_id=layout["layout_id"], keys=keys))
    record(ROOT / layout_sources["source"], ROOT / layout["output"],
           module=layout["module"], layout_id=layout["layout_id"],
           symbol=layout["symbol"], source_range=layout["source_range"], viewBox=layout_sources["view_box"])
(OUT / "keyboard-653-keys.json").write_text(json.dumps(layouts[0]["keys"], indent=2), encoding="utf-8")
(OUT / "keyboard-653-layouts.json").write_text(json.dumps(layouts, indent=2), encoding="utf-8")
(OUT / "keyboard-sources.rs").write_text(
    "// Generated from original Customize groupList exports; layouts are edition independent.\n&[\n"
    + "".join(f'    ({layout["layout_id"]}, include_str!("{Path(layout["output"]).name}")),\n'
              for layout in layout_sources["layouts"])
    + "]\n", encoding="utf-8")
for name in ["keyboard-653-layout-1.svg","keyboard-653-deviceconfig.json"]:
    record(ROOT/".ref/devices/653/static/js/965.8f16fe94.chunk.js",OUT/name,purpose="Chroma regions; not Customize hit geometry")
record(ROOT / layout_sources["source"], OUT / "keyboard-653-keys.json", module=21368, viewBox=[0,0,730,340])
for name in ["keyboard-653-customize-layouts.json", "keyboard-653-layouts.json"]:
    record(ROOT / layout_sources["source"], OUT / name, modules=[13254,21368,30387], viewBox=[0,0,730,340])

# Pairing SVGs are statically extracted from CSS data URLs / React SVG paths.
# Validate both source and output hashes before merging their provenance.
pairing = json.loads((OUT / "pairing-manifest.json").read_text(encoding="utf-8"))["assets"]
for entry in pairing:
    assert digest(ROOT / entry["source"]) == entry["source_sha256"], entry["source"]
    assert digest(ROOT / entry["output"]) == entry["sha256"], entry["output"]
    assert entry["asset"] == "synapse/" + Path(entry["output"]).name
    assert not any(existing["output"] == entry["output"] for existing in records)
    records.append(entry)
# Muted source tutorials are losslessly converted offline for GPUI's native
# animated image decoder. Preserve their independently audited provenance.
for entry in json.loads((OUT / "tutorial-media-manifest.json").read_text(encoding="utf-8"))["entries"]:
    assert digest(ROOT / entry["source"]) == entry["source_sha256"], entry["source"]
    assert digest(ROOT / entry["output"]) == entry["sha256"], entry["output"]
    assert not any(existing["output"] == entry["output"] for existing in records)
    records.append(entry)
(OUT/"manifest.json").write_text(json.dumps(dict(version=1,entries=records),indent=2),encoding="utf-8")
(OUT/"embedded.rs").write_text(
    "// Generated by tools/prepare-resources.py; paths resolve beside this file.\n&[\n"
    + "".join(f'    ("synapse/{Path(r["output"]).name}", include_bytes!("{Path(r["output"]).name}") as &[u8]),\n' for r in records)
    + "]\n", encoding="utf-8")
print(f"Prepared {len(records)} assets, {len(layouts)} keyboard layouts / "
      f"{sum(len(layout['keys']) for layout in layouts)} key regions; Customize viewBox 730 x 340")
