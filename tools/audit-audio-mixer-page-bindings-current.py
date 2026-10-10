"""Static current-source receipt for the 1342 page/DSP consumer; no JS execution."""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs/re/audio-mixer-page-bindings-source-current.json"
EQ_OUTPUT = ROOT / "assets/data/audio-mixer-mic-eq-current.json"
ECHO_OUTPUT = ROOT / "assets/data/audio-mixer-echo-current.json"
SOURCES = [
    ".ref/middleware/1342/AudioMixer.cde922aae2f0fea23404.js",
    ".ref/middleware/1342/5736.fdd55044a41484df28aa.js",
    ".ref/middleware/1342/8793.8b70f445ce1f22fe05df.js",
    ".ref/devices/1342/static/js/main.2623357c.js",
    ".ref/devices/1342/static/css/main.07a2c4ed.css",
]
TOKENS = {
    SOURCES[0]: [
        "RazerT2NoiseGateEnable", "RazerT2NoiseGateThresholdControl",
        "RazerT2NoiseGateTargetGainControl", "RazerT2NoiseGateAttackTimeControl",
        "RazerT2NoiseGateReleaseTimeControl", "RazerT2CompressorEnable",
        "RazerT2CompressorThresholdControl", "RazerT2CompressorSoftKneeWidthControl",
        "RazerT2CompressorRatioControl", "RazerT2CompressorMakeUpGainControl",
        "RazerT2CompressorAttackTimeControl", "RazerT2CompressorReleaseTimeControl",
        "RazerT2VocalFadingEnable", "RazerT2VocalFadingLevelControl",
        "RazerT2KeyShifterLevelControl", "RazerT2DSPEQBandControl",
        "this.setMagicVoiceState=", "this.setMagicVoiceMode=",
        "this.setEchoOrReverbState=", "this.setEchoOrReverbRoomValue=",
        "this.setEchoOrReverbDecayTimeValue=", "this.setEchoOrReverbEchoGainValue=",
        "this.setEchoOrReverbEchoDelayValue=",
        "ge=[new Uint8Array([19,95,252,0,52,128,83,51,0])]",
        "ye=[new Uint8Array([19,95,252,0,52,128,226,143,0])]",
        "fe=[new Uint8Array([19,95,252,0,52,128,97,71,0])]",
        "Se=[new Uint8Array([19,95,252,0,52,128,171,133,0])]",
        "this.setMicEQLevel_MultiBand=", "const an=\"MixerMic\",sn=",
    ],
    SOURCES[1]: [
        "setNoiseGateThresholdValue(Number(r.value))",
        "setNoiseGateThresholdValue(Number(a.value))",
        "setCompressorThresholdValue(Number(r.value))",
        "setCompressorThresholdValue(Number(a.value))",
        'case"basic":0==n', 'case"threshold":1==n',
        "setNoiseGateTargetGainValue(Number(s.value))",
        "setNoiseGateAttackTimeValue(Number(l.value))",
        "setNoiseGateReleaseTimeValue(Number(c.value))",
        "setCompressorSoftKneeWidthValue(Number(s.value))",
        "setCompressorRatioValue(Number(l.value))",
        "setCompressorMakeUpGainValue(Number(c.value))",
        "setCompressorAttackTimeValue(Number(u.value))",
        "setCompressorReleaseTimeValue(Number(d.value))",
        "const o=n?Number(i):0;o>=-12&&o<=12",
        "r&&r.enabled!==n&&(yield t.setVocalFadingState(n)),!n",
        "o>=0&&o<=100&&(yield t.setVocalFadingLevelValue(o))",
        "yield t.setMicEQ(!0),yield t.setMicEQLevel_MultiBand",
        "yield(0,s.f2B)(f)",
        "r&&r.enabled!==n&&(yield t.setMagicVoiceState(n)),!n",
        "[0,1,2,3].includes(o)&&(yield t.setMagicVoiceMode(o))",
        "o[0]=Di(Number(o[0]),0,100,-43,-19)",
        "r&&r.enabled!==i&&(yield t.setEchoOrReverbState(i)),!i",
        "o[2]&&o[2]>=0&&o[2]<=1",
        'case"noiseGate":{(0,D.QY)({type:n.faB,payload:Object.assign({},E[t])})',
        'case"compressor":{(0,D.QY)({type:n.swF,payload:Object.assign({},E[t])})',
        "_.wI.MW_ACTION_FROM_LOCALSTORAGE,void 0,{noiseGate:E[t]",
        "_.wI.MW_ACTION_FROM_LOCALSTORAGE,void 0,{compressor:E[t]",
        "getEQLevel_MultiBand(),i=yield e.getMicEQLevel_MultiBand()",
        "deviceEQData.audio", "deviceEQData.mic", "ON_SET_SAVE_DEVICE_EQ",
        '"AudioAWKittyBLE"===_.jf.rzDevice.name?(M.A.deviceEQData=yield Qs()',
        'f.micBandFrequency[4*i+3]', 'f.micBandFrequency[4*i+1]',
        't.micBaicEqualizer.mode', 't.micEqualizerMode=i',
    ],
    SOURCES[2]: ["DeviceId_PlaybackMix:131074", "DeviceId_StreamMix:131073",
                 "SetDeviceVolume", "SetDeviceMute", "SetMixLevel", "SetMixEnable"],
    SOURCES[3]: ["noiseGateReducer", "ON_SET_NOISE_GATE", "compressorReducer",
                 "ON_SET_COMPRESSOR", "this.reset=()=>{const e={mode:\"custom\",frequencyBands:this.props.presetData.default}",
                 "this.changeTab=e=>", 'Y=Object.entries({default:[0,0,0,0,0,0,0,0,0,0]',
                 "deviceEqDifferent", "saveDeviceEq",
                 "voiceChangerReducer", 'type:"ON_SET_VOICE_CHANGER"',
                 'className:"wrapper-noiseGate"', 'T(QP({useMode:0==n?1:0}))',
                 'className:"wrapper-compressor"', 'I(JP({useMode:0==n?1:0}))'],
    SOURCES[4]: [
        ".sliderChart__yAxisTitle{",
        ".vertical-slider__inputCustom{-webkit-appearance:none;background:#204d19",
        "transform:rotate(-90deg);width:140px",
        ".wrapper-micEqualizer #eqBox{max-width:600px",
        ".wrapper-micEqualizer .box-showLess .sliderChart__reset-button",
        ".vertical-slider__titleCustom{",
        ".wrapper-micEqualizer .vertical-slider__tagCustom{",
    ],
}

def eq_recipe():
    source = (ROOT / SOURCES[0]).read_text(encoding="utf-8")
    start = source.index(",P=(new Uint8Array([4,95,252,0,112])")
    end = source.index(',ce="Enable Magic Voice State"', start)
    arrays = [list(map(int, item.split(','))) for item in
              re.findall(r'new Uint8Array\(\[([0-9,]+)\]\)', source[start:end])]
    writes = [item for item in arrays if item[0] == 19]
    if len(writes) != 20:
        raise ValueError("Current mic EQ needs ten enabled/zero template pairs")
    frequencies = []
    for band in range(10):
        nonzero, zero = writes[band * 2:band * 2 + 2]
        if len(nonzero) != 9 or len(zero) != 9:
            raise ValueError("Current mic EQ template size changed")
        command = 0x5ffc0070 + 4 * band
        if int.from_bytes(bytes(zero[1:5]), 'big') != command or nonzero[:5] != zero[:5]:
            raise ValueError("Current mic EQ template band order changed")
        payload = int.from_bytes(bytes(zero[5:9]), 'big')
        if payload & 1 or int.from_bytes(bytes(nonzero[5:9]), 'big') != payload | 1:
            raise ValueError("Current mic EQ zero/nonzero flag changed")
        frequencies.append(payload >> 1)
    page = (ROOT / SOURCES[3]).read_text(encoding="utf-8")
    display_match = re.search(r'\bW=(\[[0-9,e]+\]);const Y=Object.entries', page)
    if not display_match:
        raise ValueError("Current 1342 microphone preset display frequency array missing")
    display = json.loads(display_match.group(1))
    basic_match = re.search(r'z=Object.entries\(\{default:(\[[0-9,-]+\]),radiovoice:(\[[0-9,-]+\]),balanced:(\[[0-9,-]+\]),midfocused:(\[[0-9,-]+\])\}', page)
    if not basic_match:
        raise ValueError("Current microphone basic preset arrays missing")
    basic_presets = {key:json.loads(value) for key,value in zip(
        ["default","radiovoice","balanced","midfocused"],basic_match.groups())}
    return {"product_id": 1342, "data_by_band": frequencies,
            "display_frequencies": display,
            "basic_presets": basic_presets,
            "basic_expansion": [0,0,0,0,1,1,1,1,2,2],
            "enable_before_bands": True, "band_order": list(range(10)),
            "source": SOURCES[0]}


def audit():
    wrapper = (ROOT / SOURCES[0]).read_text(encoding="utf-8")
    protocol = json.loads((ROOT / "assets/data/audio-mixer-protocol.json").read_text(encoding="utf-8"))
    voice = next(item for item in protocol["properties"] if item["key"] == "magic_voice")
    for mode, symbol in enumerate(["ge", "ye", "fe", "Se"]):
        match = re.search(r'\b' + symbol + r'=\[new Uint8Array\(\[([0-9,]+)\]\)\]', wrapper)
        if not match:
            raise ValueError("Current Magic Voice template missing")
        payload = list(map(int, match.group(1).split(",")))
        if payload[:5] != [19,95,252,0,52] or payload[5] != 128 or payload[8] != 0:
            raise ValueError("Current Magic Voice fixed template changed")
        if voice["codes"][str(mode)] != int.from_bytes(bytes(payload[6:8]), "big"):
            raise ValueError("Current JS/native Magic Voice mode codes differ")
    sources = []
    for name in SOURCES:
        raw = (ROOT / name).read_bytes()
        text = raw.decode("utf-8")
        receipts = []
        for token in TOKENS[name]:
            offset = text.find(token)
            if offset < 0:
                raise ValueError(f"{name}: missing current token {token}")
            start, end = max(0, offset - 220), min(len(text), offset + len(token) + 400)
            receipts.append({
                "token": token,
                "start_utf16": len(text[:start].encode("utf-16-le")) // 2,
                "end_utf16": len(text[:end].encode("utf-16-le")) // 2,
                "original": text[start:end],
            })
        sources.append({"path": name, "sha256": hashlib.sha256(raw).hexdigest(),
                        "receipts": receipts})
    return {"product_id": 1342, "runtime_executed": False, "sources": sources}

def echo_recipe():
    wrapper = (ROOT / SOURCES[0]).read_text(encoding="utf-8")
    page = (ROOT / SOURCES[3]).read_text(encoding="utf-8")
    values = re.search(r'\bw=\{(library:\[[^}]+)\}', page)
    if not values:
        raise ValueError("Current Echo/Reverb preset object missing")
    presets = {key:json.loads(re.sub(r'(?<=[\[,])\.', '0.', bands))
               for key,bands in re.findall(r'(\w+):(\[[^\]]+\])', values.group(1))}
    table = re.search(r'je=new Uint16Array\(\[([0-9,]+)\]\)', wrapper)
    if not table:
        raise ValueError("Current Echo gain table missing")
    return {"product_id":1342, "presets":presets,
            "gain_table":list(map(int,table.group(1).split(','))),
            "default_mode":"library", "delay_ms":500,
            "source":SOURCES[0], "ui_source":SOURCES[3]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    value = audit()
    eq = eq_recipe()
    echo = echo_recipe()
    descriptor_path = ROOT / "crates/razer-pages/src/features/audio_products_data.json"
    descriptors = json.loads(descriptor_path.read_text(encoding="utf-8"))
    current = next(item for item in descriptors if item["product_id"] == 1342)
    current_eq = next(item for item in current["equalizers"] if item["key"] == "mic")
    display = [f"{int(value/1000)}k" if value >= 1000 else str(int(value)) for value in eq["display_frequencies"]]
    basic_spec = {**current_eq, "key":"mic_basic", "frequencies":["BASS","MID","TREBLE"],
                  "presets":[{"key":key,"label":next(p["label"] for p in current_eq["presets"] if p["key"]==key),"bands":values}
                             for key,values in eq["basic_presets"].items()] + [{"key":"custom","label":"CUSTOM","bands":[0,0,0]}]}
    echo_controls = [
        {"path":"/device/echoReverb/isEnabled","label":"ECHO_REVERB","kind":"toggle"},
        {"path":"/device/echoReverb/activeMode","label":"ENVIRONMENT_PRESETS","kind":"presets",
         "enabled_by":"/device/echoReverb/isEnabled",
         "options":[{"label":label,"value":key} for key,label in [
             ("arena","ARENA"),("chapel","CHAPEL"),("library","LIBRARY"),
             ("marbleroom","MARBLE_ROOM"),("smallconcerthall","SMALL_CONCERT_HALL"),("custom","CUSTOM")]]},
    ]
    for index,(label,lo,hi,step,unit) in enumerate([
        ("ROOM_SIZE",0,100,1,""),("DECAY_TIME",0.6,2.7,0.1,"s"),
        ("GAIN",0,1,0.1,""),("DELAY",110,200,1,"ms")]):
        echo_controls.append({"path":f"/device/echoReverb/modeValues/{index}",
            "label":label,"kind":"slider","min":lo,"max":hi,"step":step,"unit":unit,
            "enabled_by":"/device/echoReverb/isEnabled"})
    echo_spec = {"title":"ECHO_REVERB","controls":echo_controls}
    effects = next(item for item in current["pages"] if item["key"]=="EFFECTS")
    if arguments.check:
        if json.loads(OUTPUT.read_text(encoding="utf-8")) != value:
            raise SystemExit("1342 page source receipt is stale")
        if json.loads(EQ_OUTPUT.read_text(encoding="utf-8")) != eq:
            raise SystemExit("1342 microphone EQ source recipe is stale")
        if current_eq["frequencies"] != display:
            raise SystemExit("1342 microphone EQ display labels are stale")
        if next((item for item in current["equalizers"] if item["key"]=="mic_basic"),None) != basic_spec:
            raise SystemExit("1342 microphone basic EQ spec is stale")
        if json.loads(ECHO_OUTPUT.read_text(encoding="utf-8")) != echo:
            raise SystemExit("1342 Echo source recipe is stale")
        if next((item for item in effects["sections"] if item["title"]=="ECHO_REVERB"),None) != echo_spec:
            raise SystemExit("1342 Echo UI spec is stale")
    else:
        OUTPUT.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        EQ_OUTPUT.write_text(json.dumps(eq, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        ECHO_OUTPUT.write_text(json.dumps(echo, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        current_eq["frequencies"] = display
        current["equalizers"] = [item for item in current["equalizers"] if item["key"]!="mic_basic"]+[basic_spec]
        effects["sections"] = [item for item in effects["sections"] if item["title"]!="ECHO_REVERB"]+[echo_spec]
        current["draft"]["device"]["echoReverb"] = {"isEnabled":False,"activeMode":"library",
            "modeValues":echo["presets"]["library"],"customValues":echo["presets"]["library"]}
        descriptor_path.write_text(json.dumps(descriptors, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"product_id": 1342, "sources": len(value["sources"]),
                      "runtime_executed": False}))
