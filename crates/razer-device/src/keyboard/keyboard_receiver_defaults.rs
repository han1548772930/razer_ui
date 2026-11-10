//! Current ordinary radio keyboard default bindings and DKM identities.
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Action {
    Media(&'static str, Option<bool>),
    Keyboard(&'static str),
    PrintScreen,
    Copilot,
    Application,
    Windows(&'static str),
    Brightness(&'static str),
    Profile(Option<&'static str>),
    Sleep,
    Otfm,
    Gaming,
    Disabled,
    Layout,
    SnapTap,
    Ai,
}

#[derive(Clone, Copy)]
enum Input {
    Key,
    Dkm,
    Mouse { named: bool },
}

#[derive(Clone, Copy)]
struct Binding {
    id: &'static str,
    input: Input,
    hypershift: bool,
    action: Action,
}

impl Binding {
    fn key(id: &'static str, action: Action) -> Self {
        Self {
            id,
            input: Input::Key,
            hypershift: true,
            action,
        }
    }

    fn unshifted(mut self) -> Self {
        self.hypershift = false;
        self
    }

    fn value(self) -> Value {
        let mut row = json!({
            "inputID":self.id,"isHyperShift":self.hypershift,
            "inputType":match self.input { Input::Key=>"KeyInput",Input::Dkm=>"DKMInput",Input::Mouse {..}=>"MouseInput" }
        });
        if matches!(self.input, Input::Mouse { named: true }) {
            row["mouseInput"] = self.id.into();
        }
        let (kind, group) = match self.action {
            Action::Media(assignment, turbo) => {
                let mut group = json!({"multimediaAssignment":assignment});
                if let Some(turbo) = turbo {
                    group["useTurbo"] = turbo.into();
                }
                ("multimediaGroup", Some(group))
            }
            Action::Keyboard(key) => ("keyboardGroup", Some(json!({"key":key}))),
            Action::PrintScreen => (
                "keyboardGroup",
                Some(json!({"key":"KEY_PRINT_SCREEN","keyAssignment":"Prnt Scrn","modifiers":[]})),
            ),
            Action::Copilot => (
                "keyboardGroup",
                Some(
                    json!({"keyGroup":0,"key":"KEY_F23","modifiers":["KEY_LEFT_SHIFT","KEY_LEFT_GUI"]}),
                ),
            ),
            Action::Application => (
                "keyboardGroup",
                Some(json!({"key":"KEY_APPLICATION","modifiers":[]})),
            ),
            Action::Windows(assignment) => (
                "win8ShortcutsGroup",
                Some(json!({"windowsShortcutAssignment":assignment})),
            ),
            Action::Brightness(assignment) => (
                "backlightGroup",
                Some(json!({"backlightAssignment":assignment})),
            ),
            Action::Profile(guid) => {
                let mut group = json!({"profileNavigationAssignment":if guid.is_some(){"Specific"}else{"CycleUp"}});
                if let Some(guid) = guid {
                    group["guid"] = guid.into();
                }
                ("profileNavigationGroup", Some(group))
            }
            Action::Sleep => ("systemGroup", Some(json!({"systemAssignment":"Sleep"}))),
            Action::Otfm => ("otfmGroup", None),
            Action::Gaming => ("gameModeGroup", None),
            Action::Disabled => ("disableGroup", None),
            Action::Layout => ("layoutSwitchGroup", None),
            Action::SnapTap => ("snapTapGroup", Some(json!({"snapTapAssignment":"toggle"}))),
            Action::Ai => (
                "aiLauncherGroup",
                Some(json!({"assignment":"OPEN_PROMPT_COMPOSER","param":"chat"})),
            ),
        };
        row["outputType"] = kind.into();
        if let Some(group) = group {
            row[kind] = group;
        }
        row
    }
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct DkmKey {
    #[serde(rename = "inputID")]
    pub id: &'static str,
    #[serde(rename = "buttonKey", skip_serializing_if = "Option::is_none")]
    button: Option<&'static str>,
    pub key: u8,
}

pub(super) fn dkm_keys(product: u32) -> &'static [DkmKey] {
    const MEDIA: [DkmKey; 4] = [
        DkmKey {
            id: "DKM_KBMK_01",
            button: Some("MEDIA_PREV"),
            key: 84,
        },
        DkmKey {
            id: "DKM_KBMK_02",
            button: Some("MEDIA_PAUSE_PLAY"),
            key: 85,
        },
        DkmKey {
            id: "DKM_KBMK_03",
            button: Some("MEDIA_NEXT"),
            key: 83,
        },
        DkmKey {
            id: "DKM_KBMK_04",
            button: Some("DIAL_CLICK"),
            key: 82,
        },
    ];
    const D2: DkmKey = DkmKey {
        id: "DKM_D2",
        button: Some("BUTTON_D2"),
        key: 246,
    };
    const EXTENDED: [DkmKey; 11] = [
        MEDIA[0],
        MEDIA[1],
        MEDIA[2],
        MEDIA[3],
        DkmKey {
            id: "DKM_M_01",
            button: Some("BUTTON_M_01"),
            key: 32,
        },
        DkmKey {
            id: "DKM_M_02",
            button: Some("BUTTON_M_02"),
            key: 33,
        },
        DkmKey {
            id: "DKM_M_03",
            button: Some("BUTTON_M_03"),
            key: 34,
        },
        DkmKey {
            id: "DKM_M_04",
            button: Some("BUTTON_M_04"),
            key: 35,
        },
        DkmKey {
            id: "DKM_SB_247",
            button: Some("AI_Launcher"),
            key: 247,
        },
        D2,
        DkmKey {
            id: "DKM_D41",
            button: Some("BUTTON_D41"),
            key: 65,
        },
    ];
    match product {
        600 | 697 => &[],
        602 | 658 | 664 => &MEDIA,
        724 => &EXTENDED[..10],
        716 => &EXTENDED,
        727 => &[MEDIA[0], MEDIA[1], MEDIA[2], MEDIA[3], D2],
        717 => &[
            DkmKey {
                id: "DKM_F7",
                button: None,
                key: 247,
            },
            DkmKey {
                id: "DKM_F6",
                button: None,
                key: 246,
            },
        ],
        _ => &[],
    }
}

fn dkm(id: &'static str, action: Action, hypershift: bool) -> Binding {
    Binding {
        id,
        input: Input::Dkm,
        hypershift,
        action,
    }
}

fn scroll(id: &'static str, action: Action, hypershift: bool, named: bool) -> Binding {
    Binding {
        id,
        input: Input::Mouse { named },
        hypershift,
        action,
    }
}

fn media_keys(rows: &mut Vec<Binding>, paired: bool, shifted_next: bool) {
    for (id, assignment) in [
        ("DKM_KBMK_01", "PrevTrack"),
        ("DKM_KBMK_02", "Play"),
        ("DKM_KBMK_03", "NextTrack"),
        ("DKM_KBMK_04", "MuteVolume"),
    ] {
        rows.push(dkm(id, Action::Media(assignment, None), false));
        if paired && (id == "DKM_KBMK_04" || shifted_next && id == "DKM_KBMK_03") {
            rows.push(dkm(id, Action::Media(assignment, None), true));
        }
    }
    if !paired {
        for (id, assignment) in [
            ("DKM_KBMK_01", "PrevTrack"),
            ("DKM_KBMK_02", "Play"),
            ("DKM_KBMK_03", "NextTrack"),
            ("DKM_KBMK_04", "MuteVolume"),
        ] {
            rows.push(dkm(id, Action::Media(assignment, None), true));
        }
    }
}

fn functions(rows: &mut Vec<Binding>) {
    rows.extend([
        Binding::key("KEY_F9", Action::Otfm),
        Binding::key("KEY_F10", Action::Gaming),
        Binding::key("KEY_F11", Action::Brightness("BrightnessDown")),
        Binding::key("KEY_F12", Action::Brightness("BrightnessUp")),
    ]);
}

fn profiles(rows: &mut Vec<Binding>) {
    for (id, guid) in [
        ("KEY_HOME", "8DAAE823-0968-421C-A89E-17F4673F73F9"),
        ("KEY_PAGE_UP", "4A84281A-5993-4718-90EF-F7085DDB2B2A"),
        ("KEY_DELETE", "14E68A27-F458-4AEC-8911-75FE06F2D41B"),
        ("KEY_END", "7B184C21-010E-4B00-A079-C5E736756B6F"),
        ("KEY_PAGE_DOWN", "60F108A7-AFBB-45F1-B908-C039EA286BB4"),
    ] {
        rows.push(Binding::key(id, Action::Profile(Some(guid))));
    }
}

pub(super) fn mappings(product: u32) -> anyhow::Result<Vec<Value>> {
    anyhow::ensure!(
        super::supports(product),
        "Unreviewed receiver keyboard defaults"
    );
    let mut rows = Vec::new();
    if matches!(product, 600 | 697) {
        for (id, assignment) in [
            ("KEY_V", "MuteVolume"),
            ("KEY_B", "VolumeDown"),
            ("KEY_N", "VolumeUp"),
            ("KEY_M", "PrevTrack"),
            ("KEY_COMMA", "Play"),
            ("KEY_PERIOD", "NextTrack"),
        ] {
            rows.push(Binding::key(
                id,
                Action::Media(
                    assignment,
                    (product == 697 && matches!(assignment, "VolumeDown" | "VolumeUp"))
                        .then_some(true),
                ),
            ));
        }
        rows.extend([
            Binding::key("KEY_Y", Action::Otfm),
            Binding::key("KEY_U", Action::Gaming),
            Binding::key("KEY_G", Action::Brightness("BrightnessDown")),
            Binding::key("KEY_H", Action::Brightness("BrightnessUp")),
            Binding::key("KEY_ESC", Action::Keyboard("KEY_TILDE")),
        ]);
        for (id, key) in [
            ("KEY_1", "KEY_F1"),
            ("KEY_2", "KEY_F2"),
            ("KEY_3", "KEY_F3"),
            ("KEY_4", "KEY_F4"),
            ("KEY_5", "KEY_F5"),
            ("KEY_6", "KEY_F6"),
            ("KEY_7", "KEY_F7"),
            ("KEY_8", "KEY_F8"),
            ("KEY_9", "KEY_F9"),
            ("KEY_0", "KEY_F10"),
            ("KEY_HYPEN", "KEY_F11"),
            ("KEY_EQUAL", "KEY_F12"),
        ] {
            rows.push(Binding::key(id, Action::Keyboard(key)));
        }
        rows.extend([
            Binding::key("KEY_O", Action::PrintScreen),
            Binding::key("KEY_P", Action::Keyboard("KEY_SCROLL_LOCK")),
            Binding::key("KEY_OPEN_SQUARE_BRACKET", Action::Keyboard("KEY_PAUSE")),
            Binding::key("KEY_CLOSE_SQUARE_BRACKET", Action::Sleep),
            Binding::key("KEY_APOSTROPHE", Action::Keyboard("KEY_HOME")),
            Binding::key("KEY_SLASH", Action::Keyboard("KEY_END")),
            Binding::key("KEY_C", Action::Profile(None)),
        ]);
        if product == 697 {
            rows.push(Binding::key("KEY_INSERT", Action::Disabled));
        }
    } else if product == 717 {
        rows.push(Binding::key("KEY_TAB", Action::Layout));
        for id in ["KEY_F1", "KEY_F2", "KEY_F3"] {
            rows.push(Binding::key(id, Action::Disabled).unshifted());
        }
        rows.extend([
            Binding::key("KEY_F4", Action::Windows("CycleApps")).unshifted(),
            Binding::key("KEY_F5", Action::Media("MuteVolume", None)).unshifted(),
            Binding::key("KEY_F6", Action::Media("VolumeDown", Some(true))).unshifted(),
            Binding::key("KEY_F7", Action::Media("VolumeUp", Some(true))).unshifted(),
            Binding::key("KEY_F8", Action::Windows("DisplayBrightnessDown")).unshifted(),
            Binding::key("KEY_F9", Action::Windows("DisplayBrightnessUp")).unshifted(),
            Binding::key("KEY_F10", Action::Brightness("BrightnessDown")).unshifted(),
            Binding::key("KEY_F11", Action::Brightness("BrightnessUp")).unshifted(),
            Binding::key("KEY_F12", Action::PrintScreen).unshifted(),
            dkm("DKM_F6", Action::Copilot, false),
            dkm("DKM_F7", Action::Disabled, true),
            dkm("DKM_F7", Action::Windows("LockComputer"), false),
            Binding::key("KEY_PAGE_UP", Action::Media("PrevTrack", None)),
            Binding::key("KEY_PAGE_DOWN", Action::Media("Play", None)),
            Binding::key("KEY_HOME", Action::Media("NextTrack", None)),
            Binding::key("KEY_END", Action::Media("MuteMic", None)),
            Binding::key("KEY_RIGHT_ALT", Action::Profile(None)),
            Binding::key("KEY_ESC", Action::Disabled),
        ]);
        // This source omits keyAssignment on F12.
        let mut result: Vec<_> = rows.into_iter().map(Binding::value).collect();
        result[12]["keyboardGroup"]
            .as_object_mut()
            .unwrap()
            .remove("keyAssignment");
        return Ok(result);
    } else {
        if product == 724 {
            rows.push(scroll(
                "ScrollRight",
                Action::Brightness("BrightnessUp"),
                false,
                false,
            ));
            rows.push(scroll(
                "ScrollLeft",
                Action::Brightness("BrightnessDown"),
                false,
                false,
            ));
        }
        let paired = matches!(product, 658 | 664 | 716);
        media_keys(&mut rows, paired, product == 716);
        let reversed = matches!(product, 716 | 724 | 727);
        let turbo = reversed.then_some(false);
        let named = matches!(product, 658 | 664 | 716);
        let up = Action::Media(if reversed { "VolumeDown" } else { "VolumeUp" }, turbo);
        let down = Action::Media(if reversed { "VolumeUp" } else { "VolumeDown" }, turbo);
        if paired {
            for (id, action) in [("ScrollUp", up), ("ScrollDown", down)] {
                for shift in [false, true] {
                    rows.push(scroll(id, action, shift, named));
                }
            }
        } else {
            for shift in [false, true] {
                rows.push(scroll("ScrollUp", up, shift, named));
                rows.push(scroll("ScrollDown", down, shift, named));
            }
        }
        if matches!(product, 602 | 724) {
            rows.push(Binding::key("KEY_PAUSE", Action::Sleep));
        }
        if product == 724 {
            rows.push(Binding::key("KEY_F8", Action::Sleep));
        }
        if product == 727 {
            rows.push(Binding::key("KEY_F7", Action::PrintScreen));
            rows.push(Binding::key("KEY_F8", Action::Sleep));
        }
        functions(&mut rows);
        match product {
            602 => rows.push(Binding::key("KEY_APPLICATION", Action::Profile(None))),
            658 => {
                rows.push(Binding::key("KEY_PAUSE", Action::Sleep));
                rows.push(Binding::key("KEY_APPLICATION", Action::Profile(None)));
            }
            664 => rows.extend([
                Binding::key("KEY_INSERT", Action::PrintScreen),
                Binding::key("KEY_HOME", Action::Keyboard("KEY_SCROLL_LOCK")),
                Binding::key("KEY_PAGE_UP", Action::Media("Play", None)),
                Binding::key("KEY_PAGE_DOWN", Action::Sleep),
                Binding::key("KEY_APPLICATION", Action::Profile(None)),
            ]),
            716 | 724 | 727 => {
                rows.push(dkm("DKM_D2", Action::Copilot, false));
                if product == 716 {
                    rows.push(Binding::key("KEY_PAUSE", Action::Sleep));
                    rows.push(Binding::key("KEY_F8", Action::Sleep));
                }
                if product != 727 {
                    rows.push(dkm("DKM_D2", Action::Application, true));
                }
                if product == 724 {
                    rows.push(Binding::key("KEY_RIGHT_ALT", Action::Profile(None)));
                    rows.push(Binding::key("KEY_INSERT", Action::PrintScreen));
                }
                if product == 716 {
                    rows.push(Binding::key("KEY_LEFT_SHIFT", Action::SnapTap));
                    rows.push(dkm("DKM_SB_247", Action::Ai, false));
                    rows.push(dkm("DKM_SB_247", Action::Ai, true));
                }
                if product != 727 {
                    profiles(&mut rows);
                }
                if product == 724 {
                    rows.push(dkm("DKM_SB_247", Action::Ai, false));
                    rows.push(dkm("DKM_SB_247", Action::Ai, true));
                    rows.push(Binding::key("KEY_LEFT_SHIFT", Action::SnapTap));
                }
                if product == 727 {
                    rows.push(Binding::key("KEY_LEFT_SHIFT", Action::SnapTap));
                    rows.push(Binding::key("KEY_APPLICATION", Action::Profile(None)));
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(rows.into_iter().map(Binding::value).collect())
}
