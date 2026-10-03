"""Prepare current official tray art as embedded RGBA and SVG assets."""
import hashlib
from pathlib import Path
from PIL import Image, IcoImagePlugin


def prepare(root, out):
    sources = root / '.ref/host-4.0.827/electron/resources/images'
    records = []
    def record(source, target, **extra):
        records.append(dict(source=source.relative_to(root).as_posix(),
                            output=target.relative_to(root).as_posix(),
                            source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                            sha256=hashlib.sha256(target.read_bytes()).hexdigest(), **extra))
    for name, source, size in [(f'icon-{size}', 'rzAppEngine.ico', size) for size in (16, 20, 24, 40, 48, 64)] + [('icon', 'rzAppEngine.ico', 32),
                                ('app', 'rzAppEngine.ico', 20),
                                ('gear-light', 'gear-black.png', 20),
                                ('gear-dark', 'gear-white.png', 20),
                                ('user-light', 'user-black.png', 20),
                                ('user-dark', 'user-white.png', 20)]:
        original = sources / source
        target = out / f'tray-{name}.rgba'
        if original.suffix == '.ico':
            with original.open('rb') as stream:
                # The ICO has purpose-made small frames. Its default PNG frame
                # is actually 512px despite a 256px directory entry.
                ico = IcoImagePlugin.IcoFile(stream)
                assert (size, size) in ico.sizes()
                prepared = ico.getimage((size, size)).convert('RGBA')
                assert prepared.size == (size, size)
                conversion = 'exact-ico-frame'
        else:
            with Image.open(original) as image:
                prepared = image.convert('RGBA').resize((size, size), Image.Resampling.LANCZOS)
                conversion = 'lanczos'
        target.write_bytes(prepared.tobytes())
        record(original, target, width=size, height=size, format='rgba8', conversion=conversion)
    source = root / '.ref/applications/systray/systrayv2/static/media/logo_synapse.d685ec10.svg'
    target = out / 'tray-synapse.svg'
    target.write_bytes(source.read_bytes())
    record(source, target)
    # Host common.js resolves rzAppEngine.png outside Windows. Preserve its
    # own pixels instead of substituting any Windows ICO frame.
    source = sources / 'rzAppEngine.png'
    target = out / 'tray-native.rgba'
    with Image.open(source) as image:
        assert image.size == (16, 16)
        target.write_bytes(image.convert('RGBA').tobytes())
    record(source, target, width=16, height=16, format='rgba8', conversion='exact-png')
    return records
