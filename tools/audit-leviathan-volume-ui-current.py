"""Statically validate current 1352 endpoint-volume UI receipts and consumers.

Reads current acquired JavaScript and maintained Rust only; never executes
downloaded JavaScript, an application, a DLL or device operations.
"""
from pathlib import Path
import hashlib
import json
import sys

ROOT = Path(__file__).resolve().parent.parent
RECEIPTS = ROOT / "docs/re/simple-audio-volume-current-evidence.json"
OUTPUT = ROOT / "docs/re/leviathan-volume-ui-current-evidence.json"


def main():
    native = json.loads(RECEIPTS.read_text(encoding="utf-8"))
    sources = []
    for source in native["sources"]:
        if source["path"].startswith((".ref/middleware/1352/", ".ref/devices/1352/")):
            raw = (ROOT / source["path"]).read_bytes()
            assert hashlib.sha256(raw).hexdigest() == source["sha256"], source["path"]
            text = raw.decode("utf-8").encode("utf-16-le")
            for locator in source["locators"]:
                acquired = text[locator["start_utf16"] * 2:locator["end_utf16"] * 2].decode("utf-16-le")
                assert acquired == locator["text"], (source["path"], locator["anchor"])
            sources.append(source)
    assert len(sources) == 3, "Current factory, middleware and mounted UI receipts are required"
    joined = "\n".join(locator["text"] for source in sources for locator in source["locators"])
    for token in ["registerAudioServiceEvent:!0", "const t=\"getAudioDeviceId\"",
                  "t===o.containerId", "type:t", "r.id", "name:e",
                  "e.includes(o.device.productName)", "class XU extends", "(jU,{})",
                  "volume:{isEnabled:!!e,value:e}", "simpleSetSpeakerVolume(n,e,t)",
                  "e.simpleService?k(!t.isEnabled,t.value)", "e.getSpeakerVolume().then"]:
        assert token in joined, f"Current volume source gate missing: {token}"
    gates = {
        "crates/razer-pages/src/features/mod.rs": ["AudioVolumeCompletion", "AudioVolumeOperation", "AudioVolumeReply", "AudioVolumeRequest"],
        "crates/razer-pages/src/features/audio_volume.rs": ["impl EventEmitter<AudioVolumeRequest>", "edit_revision", "endpoint_current", "self.volume.visible", "self.volume.pending = None", "AudioProductChanged"],
        "crates/razer-pages/src/features/audio_products.rs": ["mod audio_volume", "SliderEvent::Release", "commit_volume", "toggle_volume", "volume_status"],
        "crates/razer-pages/src/features/source_workspace.rs": ["sync_audio_volume_activity", "audio_volume_cancellation", "AudioVolumeRequested"],
        "crates/razer-pages/src/features/product_workspace.rs": ["finish_audio_volume", "audio_volume_request_matches", "AudioVolumeRequested"],
        "crates/razer-pages/src/features/workspace.rs": ["AudioVolumeRequested"],
        "crates/razer-shell/src/shell/audio_volume.rs": ["PAGE_AUDIO_QUEUE", "AudioDevices", "AudioVolumeRead", "AudioVolumeWrite", "check_cancel", "endpoint.kind == \"speaker\"", "endpoint.container == container", "AudioVolumeCompletion", "write.previous.result", "write.hresult >= 0"],
        "crates/razer-shell/src/shell.rs": ["mod audio_volume", "request_audio_volume"],
    }
    consumers = []
    for file, tokens in gates.items():
        raw = (ROOT / file).read_bytes()
        text = raw.decode("utf-8")
        # Formatting must not affect token gates; generated receipts still hash
        # exact bytes so a later check identifies stale consumer evidence.
        compact = "".join(text.split())
        for token in tokens:
            assert "".join(token.split()) in compact, (file, token)
        consumers.append({"path": file, "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)})
    evidence = {
        "schema_version": 1,
        "product_id": 1352,
        "method": "Current source hashes and UTF16 locators verified independently; maintained Rust consumers checked statically",
        "source_receipts": sources,
        "native_volume_evidence": "docs/re/simple-audio-volume-current-evidence.json",
        "consumers": consumers,
        "source_semantics": {
            "route": "1352 factory enables simpleService; current mounted LG contains connected XU via jU",
            "endpoint": "getAudioDeviceId filters exact speaker type, preserves enumeration order and takes first exact containerId; useVirtualAudioChannel is absent in this factory",
            "initial_read": "getSpeakerVolume success maps isEnabled=!muted,value=volume through MW changed action; default maxTry=8,delayInMs=2000",
            "write": "XU changeValue derives isEnabled=!!value; toggle preserves value; reducer emits enabled/value intent, task audioProtocol reaches simpleSetSpeakerVolume(endpointId,!isEnabled,value)",
        },
        "implementation": {
            "initial_observation": "Active sound page plus real physical owner emits typed read. Real read changes visible volume overlay and slider only; local snapshot/draft and persisted profile are untouched",
            "submission": "Pointer Release and toggle enqueue actual typed write; real endpoint .id comes from AudioDevices, never USB ID or ContainerId. Local restore/default state never emits a setter",
            "lifetime": "Epoch, edit revision, cancellation token and retained pending slot discard stale replies, cancel not-yet-submitted requests, and replay latest queued explicit write. Application workers serialize; service retains per-endpoint process lock",
            "confirmation": "Real read/write payloads validate exact endpoint ID, range, previous success, HRESULT/result and requested-volume submission relation. Source response, partial mutations, readback, route expiry and shutdown warning remain distinct",
            "platform": "UI/controller use IPC; Windows Core Audio adapter is isolated in service; unsupported platform errors remain explicit",
        },
        "gaps": [
            "Original name.includes(rzDevice.device.productName) fallback needs a live native productName field; current model catalog naming is not that observation",
            "Original absent-endpoint and source callback-failure retry branches are not fully equivalent: false getter records retry nine times, current adapter errors and initial endpoint mismatch fail explicitly",
            "Original audio service registration/callback notifications and the shared task memory/version/global-lock lifecycle are not fully connected to this page",
            "Original Range keyboard immediate-change branch is not implemented by gpui-kit Slider; pointer release and toggle are implemented",
            "Other 1352 sound panels and other products are outside this verified volume consumer; no whole-product completion claim",
        ],
        "runtime_acceptance": "Not executed: application, DLL, tests and device operations prohibited during development",
    }
    body = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
    if "--check" in sys.argv:
        assert OUTPUT.read_text(encoding="utf-8") == body, "Stale current 1352 volume UI evidence"
    else:
        OUTPUT.write_text(body, encoding="utf-8")
    print(json.dumps({"product_id": 1352, "sources": len(sources), "consumers": len(consumers), "runtime_executed": False}))


if __name__ == "__main__":
    main()
