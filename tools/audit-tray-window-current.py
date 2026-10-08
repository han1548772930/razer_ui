"""Audit current tray bounds and framework provenance statically.

--fetch retrieves version-pinned framework source as inert bytes only.
--write records reviewed source excerpts. No PE, DLL or vendor JS is executed.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT / ".ref/framework/electron-41.2.0"
FILES = ["DEPS", "shell/browser/ui/win/notify_icon.cc",
         "shell/browser/api/electron_api_base_window.cc",
         "shell/browser/native_window_views.cc"]
OUTPUT = ROOT / "docs/re/tray-window-current-evidence.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def excerpt(path, start, end):
    content = (ROOT / path).read_text(encoding="utf-8")
    offset = content.index(start)
    stop = content.index(end, offset) + len(end)
    return dict(path=path, start=offset, end=stop, source=content[offset:stop],
                sha256=digest((ROOT / path).read_bytes()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    receipts = []
    for file in FILES:
        url = "https://raw.githubusercontent.com/electron/electron/v41.2.0/" + file
        path = FRAMEWORK / file
        if args.fetch:
            with urlopen(url, timeout=30) as response:
                data = response.read()
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        data = path.read_bytes()
        receipts.append(dict(path=path.relative_to(ROOT).as_posix(), url=url,
                             bytes=len(data), sha256=digest(data)))
    pe_path = ROOT / ".work/tray-native-current/win-unpacked/RazerAppEngine.exe"
    pe = pe_path.read_bytes()
    assert digest(pe) == "9b796816dc65318d496a0caa10eebf8b0814987e92cb19607fd406e92fb5e0dc"
    assert b"Electron/41.2.0" in pe and b"Chrome/146.0.7680.179" in pe
    assert re.search(r"'chromium_version':\s*'146\.0\.7680\.179'", (FRAMEWORK / "DEPS").read_text())
    chain_path = ".ref/host-4.0.827/ARCHIVE-CHAIN.json"
    chain = json.loads((ROOT / chain_path).read_text(encoding="utf-8"))
    geometry = json.loads((ROOT / "docs/re/tray-account-current-evidence.json")
                          .read_text(encoding="utf-8"))["geometry"]
    source = (ROOT / geometry["path"]).read_text(encoding="utf-8")
    assert source[geometry["start"]:geometry["end"]] == geometry["source"]
    assert digest((ROOT / geometry["path"]).read_bytes()) == geometry["sha256"]
    nodes = [
        excerpt(geometry["path"], 'a=e=>{if(e){', '}}};async function c'),
        excerpt(".ref/host-4.0.827/electron/main.js", 'l.themeSource="dark"', ',en='),
        excerpt(".ref/host-4.0.827/electron/main.js", 'case D.SET_BOUNDS:', 'break}case"getBounds"'),
        excerpt(".ref/host-4.0.827/electron/preload.js", 'setBounds:(...t)=>', 'getUserApps:'),
        excerpt(".ref/host-4.0.827/electron/lib/common.js", 'resizable:b(r.resizable,!0)', ',canMinimize:'),
        excerpt(".ref/host-4.0.827/electron/lib/common.js",
                'module.exports.convertFeatureToWindowOptions=r=>', '},module.exports.isWindowOS='),
        excerpt(".ref/host-4.0.827/electron/lib/common.js", 'b=(e,t)=>', '},S=e=>'),
        excerpt(".ref/host-4.0.827/electron/lib/common.js", 'hasShadow:0!==b(r.hasShadow,1)', ',isLaunchBackgroundOnStart:'),
        excerpt(".ref/host-4.0.827/electron/components/Tab/common.js",
                'function ve(o,n,t){', 'y=new i({...t,transparent:w||t.transparent,parent:u})'),
        excerpt(".ref/host-4.0.827/electron/components/Tab/LeftSystray.js",
                'createSystrayWindow(t){', 'this.alignWindow()'),
        excerpt(".ref/framework/electron-41.2.0/shell/browser/api/electron_api_base_window.cc",
                'void BaseWindow::SetBounds(', 'window_->SetBounds(bounds, animate);\n}'),
        excerpt(".ref/framework/electron-41.2.0/shell/browser/native_window_views.cc",
                'void NativeWindowViews::SetBounds(', 'widget()->SetBounds(LogicalToWidgetBounds(bounds));\n}'),
        excerpt(".ref/framework/electron-41.2.0/shell/browser/native_window_views.cc",
                'bool NativeWindowViews::CanResize() const {', 'return resizable_;\n#endif\n}'),
        excerpt(".ref/framework/electron-41.2.0/shell/browser/native_window_views.cc",
                '// On Windows we rely on the CanResize()', 'thick_frame_ = false;'),
        excerpt(".ref/framework/electron-41.2.0/shell/browser/native_window_views.cc",
                '// The given window is most likely not rectangular', 'params.shadow_type = InitParams::ShadowType::kNone;'),
        excerpt(".ref/framework/electron-41.2.0/shell/browser/ui/win/notify_icon.cc",
                'void NotifyIcon::PopUpContextMenu(', 'ui::mojom::MenuSourceType::kMouse);\n}'),
    ]
    snippets = [node["source"] for node in nodes]
    assert any('resizable:!1' in node for node in snippets)
    assert any('SetMinimumSize(bounds.size())' in node for node in snippets)
    assert any('views::MenuRunner' in node for node in snippets)
    assert 'width:e.WorkRect.width' in snippets[0]
    assert 'Math.abs(Math.abs(e.WorkRect.right)-Math.abs(e.WorkRect.left))' in snippets[0]
    assert any('"boolean"==typeof e?e:o' in node for node in snippets)
    assert any('hasShadow:0!==b(r.hasShadow,1)' in node for node in snippets)
    assert any('has_frame() ? resizable_ && thick_frame_ : resizable_' in node for node in snippets)
    assert any('frame:l,transparent:6===+r.policy||8===+r.policy' in node
               and 'l=6!==+r.policy&&8!==+r.policy' in node for node in snippets)
    implementation = (ROOT / "src/shell/tray/windows.rs").read_text(encoding="utf-8")
    assert 'let height = (60. * scale)' in implementation
    assert 'let y = tray_y - height;' in implementation
    assert 'let right_gap = (10. * scale)' in implementation
    assert 'let primary_width = (work.right.abs() - work.left.abs()).abs();' in implementation
    assert 'y = work.bottom - height' not in implementation
    assert 'AppsUseLightTheme' not in implementation
    assert 'if native::visible(window)' in implementation
    tray = (ROOT / "src/shell/tray.rs").read_text(encoding="utf-8")
    assert 'fn menu_is_dark(_cx: &App) -> bool' in tray
    assert '.with_menu(Box::new(native_menu(cx)?))' in tray
    evidence = dict(schema_version=1, framework_sources=receipts, nodes=nodes,
                    geometry=geometry, archive_chain=dict(path=chain_path,
                    sha256=digest((ROOT / chain_path).read_bytes()),
                    outer_sha256=chain["outer_exe"]["sha256"],
                    inner_sha256=chain["internal_exe"]["sha256"]),
                    executable=dict(path=pe_path.relative_to(ROOT).as_posix(),
                    bytes=len(pe), sha256=digest(pe), electron="41.2.0",
                    chromium="146.0.7680.179", execution="none",
                    extraction="7-Zip static archive read: current verified internal package entry win-unpacked/RazerAppEngine.exe"),
                    conclusion=[
                    "Signed-out renderer requests 360x60; the source's singular .app query misses .apps.",
                    "Non-resizable Electron windows reset min/max constraints to requested setBounds size.",
                    "Signed-out placement has a 10px right-edge gap and no account/body vertical clamp.",
                    "The renderer's first horizontal clamp uses primary WorkRect width, not monitor width.",
                    "Host createWindow preserves resizable:false; Windows frameless CanResize reads resizable_.",
                    "hasShadow:false is converted to true by strict 0!==boolean; Electron still disables shadow for translucent frameless windows.",
                    "Host forces dark theme; retained HMENU and its native-palette icon choice are user-accepted platform differences."],
                    remaining=["Mixed-monitor DPI and real windows are not validated; renderer DIP coordinates and partly divided monitor fields are not a proven pixel-equivalent match to the physical-coordinate adapter.",
                    "Account sessions, dynamic body height and native menu visual parity remain incomplete."],
                    runtime_validation="not_run")
    serialized = json.dumps(evidence, indent=2, ensure_ascii=False) + "\n"
    if args.write:
        OUTPUT.write_text(serialized, encoding="utf-8")
    else:
        assert OUTPUT.read_text(encoding="utf-8") == serialized, "Review evidence before --write"
    print("Current tray: PE version chain, source bounds, forced theme and retained menu adapter checked statically.")


if __name__ == "__main__":
    main()
