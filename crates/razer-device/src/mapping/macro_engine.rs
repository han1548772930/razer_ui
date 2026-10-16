//! Current macro main.3f4b9604 factory13139 convertMacroDataToAppEngine,
//! matching PID70 MW2520 factory67306. The 150 current macro629 key rows match
//! the shared table on all fields used here. Pure compilation, never execution.
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|v| v.trim().parse().ok()))
        .filter(|v| v.is_finite())
        .unwrap_or(0.)
}
fn round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// `geometry` is the actual recorder monitor snapshot. Imported relative
/// coordinates can use their explicitly retained geometry; none is invented.
pub fn compile_event(
    event: &Value,
    movement_mode: u8,
    geometry: Option<&Value>,
) -> Result<Vec<Value>> {
    let Some(kind) = event["Type"].as_u64() else {
        return Ok(Vec::new());
    };
    let values = match kind {
        0 if event["Number"].is_object() => vec![json!({"type":"randomDelay",
            "minMs":round(1000.*number(&event["Number"]["min"])),
            "maxMs":round(1000.*number(&event["Number"]["max"]))})],
        0 => vec![json!({"type":"delay","ms":round(1000.*number(&event["Number"]))})],
        1 => {
            let Some(makecode) = event["KeyEvent"]["Makecode"].as_u64() else {
                return Ok(Vec::new());
            };
            let keys = super::mapping_engine::source_keys(70)
                .context("Missing current macro key table")?;
            let key_type = event["KeyEvent"]["Type"].as_str();
            let override_key = match makecode {
                285 => Some("KEY_RIGHT_CTRL"),
                310 => Some("KEY_RIGHT_SHIFT"),
                312 => Some("KEY_RIGHT_ALT"),
                _ => None,
            };
            let code = if (256..=258).contains(&makecode) {
                makecode - 240
            } else {
                makecode
            };
            let key = keys.iter().find(|key| {
                if let Some(id) = override_key {
                    return key.input_id == Some(id);
                }
                key.key_code.parse::<u64>().ok() == Some(code)
                    && ((256..=258).contains(&makecode)
                        || key_type.is_none_or(|kind| key.kind == kind))
            });
            let state = &event["KeyEvent"]["State"];
            let mut scancode = key.map(|key| json!(key.scancode)).unwrap_or(Value::Null);
            if !state.is_null() && number(&event["ScanCode"]) != 0. && event["ScanCode"] != scancode
            {
                if let Some(key) = keys
                    .iter()
                    .find(|key| event["ScanCode"] == key.scancode && event["flag"] == key.flag)
                {
                    scancode = json!(key.scancode);
                }
            }
            if state.is_null() {
                vec![
                    json!({"type":"keyboard","scancode":scancode,"flag":0}),
                    json!({"type":"keyboard","scancode":scancode,"flag":1}),
                ]
            } else {
                vec![json!({"type":"keyboard","scancode":scancode,"flag":state})]
            }
        }
        2 => {
            let Some(button) = event["MouseEvent"]["MouseButton"].as_u64() else {
                return Ok(Vec::new());
            };
            let state = event["MouseEvent"]["State"].as_u64();
            match button {
                5 if state == Some(0) => vec![
                    json!({"type":"mouse","id":1}),
                    json!({"type":"mouse","id":2}),
                    json!({"type":"delay","ms":10}),
                    json!({"type":"mouse","id":1}),
                    json!({"type":"mouse","id":2}),
                ],
                5 => Vec::new(),
                6..=9 if state.unwrap_or(0) == 0 => vec![
                    json!({"type":"mouse","id":if button<8 {2048} else {1024},"data":if button==6 || button==9 {65416} else {120}}),
                ],
                6..=9 => Vec::new(),
                _ => {
                    let down = match button {
                        1 => 4,
                        2 => 16,
                        3 => 64,
                        4 => 256,
                        _ => 1,
                    };
                    match state {
                        None => vec![
                            json!({"type":"mouse","id":down*2}),
                            json!({"type":"mouse","id":down}),
                        ],
                        Some(0) => vec![json!({"type":"mouse","id":down})],
                        Some(_) => vec![json!({"type":"mouse","id":down*2})],
                    }
                }
            }
        }
        3 => {
            let buffer = event["MouseEvent"]["Buffer"]
                .as_array()
                .context("Missing recorded mouse movement buffer")?;
            if buffer.is_empty() {
                return Ok(Vec::new());
            }
            let geometry = geometry.context("Mouse movement has no recorded monitor geometry")?;
            let monitors = geometry["monitorInforBefore"]
                .as_array()
                .context("Missing recorded monitor snapshot")?;
            let first = monitors
                .first()
                .context("Empty recorded monitor snapshot")?;
            let primary = monitors
                .iter()
                .find(|monitor| monitor["isPrimaryMonitor"] == true)
                .context("Recorded primary monitor is missing")?;
            let width =
                number(&primary["MonitorRect"]["right"]) - number(&primary["MonitorRect"]["left"]);
            let height =
                number(&primary["MonitorRect"]["bottom"]) - number(&primary["MonitorRect"]["top"]);
            ensure!(
                width > 0. && height > 0.,
                "Recorded primary monitor dimensions are invalid"
            );
            let mut events = Vec::new();
            let mut residual = 0.;
            for item in buffer {
                let x = number(&item["x"]);
                let y = number(&item["y"]);
                let monitor = if x > number(&first["MonitorRect"]["right"])
                    - number(&first["MonitorRect"]["left"])
                    || y > number(&first["MonitorRect"]["bottom"])
                        - number(&first["MonitorRect"]["top"])
                    || x < 0.
                    || y < 0.
                {
                    monitors.get(1)
                } else {
                    Some(first)
                };
                let (x, y) = match movement_mode {
                    1 | 2 => (round(x / width * 65535.), round(y / height * 65535.)),
                    3 => {
                        let sx = monitor.map(|v| number(&v["dpiScaleX"])).unwrap_or(1.);
                        let sy = monitor.map(|v| number(&v["dpiScaleY"])).unwrap_or(1.);
                        ensure!(sx > 0. && sy > 0., "Recorded monitor DPI scale is invalid");
                        (round(x / sx), round(y / sy))
                    }
                    0 => (0., 0.),
                    _ => bail!("Unknown source mouse movement mode"),
                };
                events.push(json!({"type":"mouseMove","x":x,"y":y}));
                let time = number(&item["time"]);
                let mut ms = round(time);
                residual += time - ms;
                if residual > 1. {
                    ms += 1.;
                    residual -= 1.;
                } else if residual < -1. {
                    ms -= 1.;
                    residual += 1.;
                }
                if ms > 0. {
                    events.push(json!({"type":"delay","ms":ms}));
                }
            }
            events
        }
        4 => match event["Text"].as_str().filter(|text| !text.is_empty()) {
            Some(text) => vec![
                json!({"type":"clipboard","id":"text","text":text}),
                json!({"type":"delay","ms":500}),
            ],
            None => Vec::new(),
        },
        5 => match event["TextCommand"]
            .as_str()
            .filter(|text| !text.is_empty())
        {
            Some(text) => vec![
                json!({"type":"launch","path":if text.contains("taskmgr") {"cmd /c start taskmgr"} else {text}}),
                json!({"type":"delay","ms":50}),
            ],
            None => Vec::new(),
        },
        6 if number(&event["LoopEvent"]["State"]) != 0. => vec![json!({"type":"loop","flag":1})],
        6 => vec![json!({"type":"loop","flag":0,"repeat":event["Number"]})],
        7 => match event["guid"].as_str().filter(|guid| !guid.is_empty()) {
            Some(guid) => vec![json!({"type":"macro","guid":guid,"flag":0})],
            None => Vec::new(),
        },
        10 => {
            if event["RadioIndex"] == 1 {
                // Website launching depends on Internet and is outside the
                // local-only product scope. Never publish an executable URL.
                bail!("Internet website macro actions are excluded from local macro publication");
            }
            if event["RadioIndex"] != 0 {
                return Ok(Vec::new());
            }
            let Some(path) = event["Content0"].as_str().filter(|v| !v.is_empty()) else {
                return Ok(Vec::new());
            };
            let executable = path
                .rsplit('.')
                .next()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
            vec![
                json!({"type":"launch","path":if executable{path.to_owned()}else{format!("cmd /c \"{path}\"")},"startHidden":!executable,"pathToCheck":path}),
                json!({"type":"delay","ms":50}),
            ]
        }
        _ => bail!("Unresolved source macro event type {kind}"),
    };
    Ok(values)
}

/// Source6120 Ke inserts scheduling delays during publication, without
/// changing the local editor's recorded delay rows.
pub fn publication_events(events: &[Value]) -> Vec<Value> {
    let mut count = 0;
    let mut result = Vec::new();
    for (index, event) in events.iter().enumerate() {
        count += 1;
        result.push(event.clone());
        if index + 1 == events.len() {
            if event["type"] != "delay" || number(&event["ms"]) < 3. {
                result.push(json!({"type":"delay","ms":20,"kind":"virtual"}));
            }
        } else if count == 5 {
            count = 0;
            if events[index + 1]["type"] != "delay" {
                result.push(json!({"type":"delay","ms":3,"kind":"virtual"}));
            }
        } else if event["type"] == "delay" || event["type"] == "mouseMove" {
            count = 0;
        }
    }
    result
}

/// Current macro UI Js/qs: remove editor-only fields, add scheduling delays to
/// the ordinary lane, then hash the complete engine including phased lanes.
pub fn publication_documents(documents: &[Value]) -> Result<Vec<Value>> {
    documents
        .iter()
        .map(|document| {
            let mut document = document.clone();
            let object = document
                .as_object_mut()
                .context("Macro document is not an object")?;
            for key in ["actionLoad", "deviceList", "saved", "macroEvents"] {
                object.remove(key);
            }
            let engine = document
                .get_mut("appEngine")
                .context("Macro document has no compiled engine")?;
            let events = engine["events"]
                .as_array()
                .context("Macro engine has no ordinary event lane")?;
            engine["events"] = publication_events(events).into();
            engine["hash"] = super::mapping_engine_hash::source_hash(engine)?.into();
            Ok(document)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_click_and_scroll_preserve_source_order() {
        assert_eq!(
            compile_event(
                &json!({"Type":2,"MouseEvent":{"MouseButton":1,"State":null}}),
                0,
                None
            )
            .unwrap(),
            vec![
                json!({"type":"mouse","id":8}),
                json!({"type":"mouse","id":4})
            ]
        );
        assert!(
            compile_event(
                &json!({"Type":2,"MouseEvent":{"MouseButton":8,"State":1}}),
                0,
                None
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            compile_event(
                &json!({"Type":2,"MouseEvent":{"MouseButton":9,"State":0}}),
                0,
                None
            )
            .unwrap(),
            vec![json!({"type":"mouse","id":1024,"data":65416})]
        );
    }
    #[test]
    fn publication_delays_do_not_mutate_saved_editor_events_or_phases() {
        let keys = vec![json!({"type":"keyboard","scancode":30,"flag":0}); 6];
        let documents = vec![
            json!({"guid":"one","macroEvents":[{"Type":1}],"saved":true,"appEngine":{"events":keys,"pressEvents":[{"type":"mouse","id":1}],"mouseMoveType":"none"}}),
        ];
        let result = publication_documents(&documents).unwrap();
        assert_eq!(
            documents[0]["appEngine"]["events"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            result[0]["appEngine"]["events"].as_array().unwrap().len(),
            8
        );
        assert_eq!(
            result[0]["appEngine"]["events"][5],
            json!({"type":"delay","ms":3,"kind":"virtual"})
        );
        assert_eq!(
            result[0]["appEngine"]["events"][7],
            json!({"type":"delay","ms":20,"kind":"virtual"})
        );
        assert_eq!(
            result[0]["appEngine"]["pressEvents"],
            documents[0]["appEngine"]["pressEvents"]
        );
        assert!(result[0].get("macroEvents").is_none());
        assert_eq!(
            result[0]["appEngine"]["hash"],
            super::super::mapping_engine_hash::source_hash(&result[0]["appEngine"]).unwrap()
        );
    }
    #[test]
    fn website_never_becomes_native_launch() {
        assert!(
            compile_event(
                &json!({"Type":10,"RadioIndex":1,"Content1":"example.com"}),
                0,
                None
            )
            .is_err()
        );
    }
}
