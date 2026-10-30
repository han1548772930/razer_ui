//! Current movement callback arithmetic and publication reset.
//! The page owns its separate 150 ms visible-speed expiry.
use anyhow::{Context as _, ensure};
use serde::Deserialize;

pub fn is_direct_product(product_id: u32, event_product_id: u32) -> anyhow::Result<bool> {
    let capability =
        super::capability(product_id).context("Dynamic movement product is unsupported")?;
    // DeviceInfo contains bleId only for the current PID226 branch. Sender
    // aliases and receiver transport capabilities are separate identities.
    Ok(event_product_id == capability.wired_pid()
        || capability.dongle_pid() == Some(event_product_id)
        || product_id == 226 && event_product_id == 228)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireTick {
    Number(f64),
    Text(String),
}

#[derive(Clone, Default)]
struct Tick {
    number: f64,
    text: Option<String>,
}
impl Tick {
    fn decode(wire: WireTick) -> Self {
        let (number, text) = match wire {
            WireTick::Number(number) => (number, None),
            WireTick::Text(text) => (crate::ecmascript_number::from_string(&text), Some(text)),
        };
        Self { number, text }
    }

    fn is_numeric_zero(&self) -> bool {
        self.text.is_none() && self.number == 0.
    }

    fn is_after(&self, previous: &Self) -> bool {
        match (&self.text, &previous.text) {
            // Native ticks are decimal strings. JSON.parse preserves their
            // type: string/string `>` compares lexically, subtraction does not.
            (Some(current), Some(previous)) => {
                current.encode_utf16().cmp(previous.encode_utf16()).is_gt()
            }
            _ => self.number > previous.number,
        }
    }
}

#[derive(Clone)]
pub struct Speed {
    last_tick: Tick,
    speed: f64,
}

impl Speed {
    pub fn new(product_id: u32) -> anyhow::Result<Self> {
        ensure!(
            super::PRODUCTS.contains(&product_id),
            "Dynamic movement product is unsupported"
        );
        Ok(Self {
            last_tick: Tick::default(),
            speed: 0.,
        })
    }

    pub fn observe(&mut self, json_event: &str) -> anyhow::Result<()> {
        self.observe_filtered(json_event, true)
    }

    pub fn observe_filtered(&mut self, json_event: &str, admitted: bool) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            data: String,
            time_tick: Option<serde_json::Value>,
        }
        #[derive(Deserialize)]
        #[serde(tag = "type")]
        enum Data {
            #[serde(rename = "mouseMove")]
            Move { x: f64, y: f64 },
            #[serde(other)]
            Other,
        }
        let rows: Vec<Row> = serde_json::from_str(json_event)?;
        let (mut x, mut y, mut ticks) = (0., 0., 0.);
        let mut next = self.clone();
        for row in rows {
            if !admitted {
                // The callback reads data.type before its product gate. A
                // null data object fails there; movement fields remain unused.
                let data: Option<serde::de::IgnoredAny> = serde_json::from_str(&row.data)?;
                ensure!(data.is_some(), "Dynamic movement data is null");
                continue;
            }
            let Data::Move { x: dx, y: dy } = serde_json::from_str(&row.data)? else {
                continue;
            };
            let tick = Tick::decode(serde_json::from_value(
                row.time_tick.context("Dynamic movement omits timeTick")?,
            )?);
            ensure!(
                dx.is_finite() && dy.is_finite(),
                "Dynamic movement contains nonfinite coordinates"
            );
            if !next.last_tick.is_numeric_zero() && tick.is_after(&next.last_tick) {
                x += dx.abs();
                y += dy.abs();
                ticks += tick.number - next.last_tick.number;
            }
            next.last_tick = tick;
        }
        if ticks > 0. {
            next.speed = (x.powi(2) + y.powi(2)).sqrt() / (ticks / 1e4);
        }
        next.speed = if next.speed.is_nan() { 0. } else { next.speed.abs() };
        *self = next;
        Ok(())
    }

    pub fn publish(&mut self) -> f64 {
        let speed = self.speed;
        self.speed = 0.;
        self.last_tick = Tick::default();
        speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_input_direct_ids_follow_device_info_without_transport_aliases() {
        for (product_id, dongle_id, ble_id) in [
            (190, 191, None),
            (192, 193, None),
            (196, 197, None),
            (222, 223, None),
            (226, 227, Some(228)),
            (229, 230, None),
            (239, 240, None),
        ] {
            for event_product_id in [
                164, 179, 190, 191, 192, 193, 196, 197, 222, 223, 226, 227, 228, 229, 230, 239,
                240, 241,
            ] {
                assert_eq!(
                    is_direct_product(product_id, event_product_id).unwrap(),
                    event_product_id == product_id
                        || event_product_id == dongle_id
                        || ble_id == Some(event_product_id),
                    "{product_id}/{event_product_id}",
                );
            }
        }
        assert!(is_direct_product(228, 228).is_err());
    }

    #[test]
    fn dynamic_filtered_callbacks_preserve_product_normalization_and_movement_history() {
        for product_id in [190, 192, 196, 222, 226, 229, 239] {
            let mut speed = Speed::new(product_id).unwrap();
            speed.observe(r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#).unwrap();
            speed
                .observe_filtered(
                    r#"[{"timeTick":{"unknown":true},"data":"{\"type\":\"mouseMove\"}"}]"#,
                    false,
                )
                .unwrap();
            assert_eq!(speed.clone().publish(), 10.);
            assert!(
                speed
                    .observe_filtered(r#"[{"data":"broken"}]"#, false)
                    .is_err()
            );
            assert!(
                speed
                    .observe_filtered(r#"[{"data":"null"}]"#, false)
                    .is_err()
            );
            speed
                .observe(
                    r#"[{"timeTick":"30000","data":"{\"type\":\"mouseMove\",\"x\":9,\"y\":12}"}]"#,
                )
                .unwrap();
            assert_eq!(speed.publish(), 15.);
        }
    }

    // Current MW modules 32807 (six products) and 14786 (226) provide the
    // independent callback cases retained in the shared monitoring audit.
    struct Case {
        name: &'static str,
        events: &'static str,
        speed: f64,
    }

    #[test]
    fn all_current_products_preserve_native_tick_types_and_callback_arithmetic() {
        let cases = [
            Case {
                name: "unchanged relative deltas",
                events: r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#,
                speed: 5.,
            },
            Case {
                name: "equal ticks",
                events: r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                speed: 0.,
            },
            Case {
                name: "string digit growth",
                events: r#"[{"timeTick":"9","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"10","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                speed: 0.,
            },
            Case {
                name: "string zero",
                events: r#"[{"timeTick":"0","data":"{\"type\":\"mouseMove\",\"x\":10,\"y\":4}"},{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":12,\"y\":6}"}]"#,
                speed: 13.416407864998739,
            },
            Case {
                name: "number zero",
                events: r#"[{"timeTick":0,"data":"{\"type\":\"mouseMove\",\"x\":10,\"y\":4}"},{"timeTick":10000,"data":"{\"type\":\"mouseMove\",\"x\":12,\"y\":6}"}]"#,
                speed: 0.,
            },
            Case {
                name: "number and string comparison",
                events: r#"[{"timeTick":"9","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":10,"data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                speed: 100000.,
            },
            Case {
                name: "lexical pass with negative duration",
                events: r#"[{"timeTick":"10","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"9","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                speed: 0.,
            },
            Case {
                name: "horizontal motion",
                events: r#"[{"timeTick":10000,"data":"{\"type\":\"mouseMove\",\"x\":10,\"y\":0}"},{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":12,\"y\":0}"},{"timeTick":30000,"data":"{\"type\":\"mouseMove\",\"x\":15,\"y\":0}"}]"#,
                speed: 13.5,
            },
            Case {
                name: "changing relative deltas",
                events: r#"[{"timeTick":10000,"data":"{\"type\":\"mouseMove\",\"x\":10,\"y\":4}"},{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":12,\"y\":6}"},{"timeTick":30000,"data":"{\"type\":\"mouseMove\",\"x\":15,\"y\":9}"}]"#,
                speed: 15.443445211480501,
            },
        ];
        for product_id in [190, 192, 196, 222, 226, 229, 239] {
            for case in &cases {
                let mut speed = Speed::new(product_id).unwrap();
                speed.observe(case.events).unwrap();
                assert_eq!(speed.publish(), case.speed, "{product_id}: {}", case.name);
            }
        }
        for product_id in [0, 70, 182, 228, u32::MAX] {
            assert!(Speed::new(product_id).is_err(), "{product_id}");
        }
    }

    #[test]
    fn callback_boundaries_and_publication_keep_the_selected_product_behavior() {
        for product_id in [190, 192, 196, 222, 226, 229, 239] {
            let mut speed = Speed::new(product_id).unwrap();
            speed
                .observe(
                    r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#,
                )
                .unwrap();
            speed
                .observe(
                    r#"[{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                )
                .unwrap();
            speed.observe("[]").unwrap();
            assert_eq!(speed.publish(), 10.);
            speed
                .observe(
                    r#"[{"timeTick":"30000","data":"{\"type\":\"mouseMove\",\"x\":9,\"y\":12}"}]"#,
                )
                .unwrap();
            assert_eq!(speed.publish(), 0.);
            speed.observe(r#"[{"timeTick":"40000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"40000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#).unwrap();
            assert_eq!(speed.publish(), 0.);
        }
    }

    #[test]
    fn invalid_callback_cannot_partially_replace_speed_or_tick() {
        for product_id in [190, 192, 196, 222, 226, 229, 239] {
            let mut speed = Speed::new(product_id).unwrap();
            speed
                .observe(
                    r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#,
                )
                .unwrap();
            for invalid in [
                r#"[{"timeTick":"15000","data":"{\"type\":\"mouseMove\",\"x\":99,\"y\":99}"},{"data":"{\"type\":\"mouseMove\",\"x\":1,\"y\":2}"}]"#,
                r#"[{"timeTick":true,"data":"{\"type\":\"mouseMove\",\"x\":1,\"y\":2}"}]"#,
                r#"[{"timeTick":null,"data":"{\"type\":\"mouseMove\",\"x\":1,\"y\":2}"}]"#,
                r#"[{"timeTick":[],"data":"{\"type\":\"mouseMove\",\"x\":1,\"y\":2}"}]"#,
                r#"[{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":1}"}]"#,
                r#"[{"timeTick":20000,"data":"broken"}]"#,
                "{}",
            ] {
                assert!(speed.observe(invalid).is_err(), "{product_id}: {invalid}");
            }
            speed.observe(r#"[{"data":"{\"type\":\"mouseDown\"}"},{"timeTick":"20000","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#).unwrap();
            assert_eq!(speed.publish(), 10.);
        }
    }

    #[test]
    fn dynamic_tick_conversion_preserves_source_string_comparison_and_number_arithmetic() {
        for product_id in [190, 192, 196, 222, 226, 229, 239] {
            for (first, second, expected) in [
                (serde_json::json!("0x10"), serde_json::json!(32), 3125.),
                (serde_json::json!("0b10000"), serde_json::json!(32), 3125.),
                (serde_json::json!("0o20"), serde_json::json!(32), 3125.),
                (serde_json::json!("0x10"), serde_json::json!("0x20"), 3125.),
                (serde_json::json!("0x9"), serde_json::json!("0x10"), 0.),
                (serde_json::json!(""), serde_json::json!(10000), 5.),
                (serde_json::json!(" \u{feff}"), serde_json::json!(10000), 5.),
                (serde_json::json!("-0"), serde_json::json!(10000), 5.),
                (serde_json::json!(0), serde_json::json!(10000), 0.),
                (serde_json::json!(-10000), serde_json::json!(0), 5.),
                (
                    serde_json::json!("\u{feff}1e4\u{3000}"),
                    serde_json::json!(20000),
                    5.,
                ),
                (serde_json::json!("-Infinity"), serde_json::json!(20000), 0.),
                (serde_json::json!("1e400"), serde_json::json!(20000), 0.),
                (serde_json::json!("NaN"), serde_json::json!(20000), 0.),
                (
                    serde_json::json!("not-a-number"),
                    serde_json::json!(20000),
                    0.,
                ),
            ] {
                let events = serde_json::json!([
                    {"timeTick":first,"data":r#"{"type":"mouseMove","x":99,"y":99}"#},
                    {"timeTick":second,"data":r#"{"type":"mouseMove","x":3,"y":4}"#},
                ]);
                let mut speed = Speed::new(product_id).unwrap();
                speed.observe(&events.to_string()).unwrap();
                assert_eq!(speed.publish(), expected, "{product_id}: {events}");
                assert_eq!(speed.publish(), 0.);
            }
        }
    }

    #[test]
    fn dynamic_nonfinite_ticks_follow_source_comparison_and_infinite_duration() {
        for product_id in [190, 192, 196, 222, 226, 229, 239] {
            // A positive infinite tick passes `l > lastTimeTick`; finite
            // distance divided by its infinite duration produces zero. NaN
            // and negative infinity fail that comparison. A subsequent finite
            // tick after negative infinity also has infinite duration.
            for (text, after_nonfinite, after_first_finite) in [
                ("NaN", 5., 5.),
                ("inf", 5., 5.),
                ("not-a-number", 5., 5.),
                ("1e400", 0., 0.),
                ("Infinity", 0., 0.),
                ("+Infinity", 0., 0.),
                ("-1e400", 5., 0.),
                ("-Infinity", 5., 0.),
            ] {
                let mut speed = Speed::new(product_id).unwrap();
                speed.observe(r#"[{"timeTick":10000,"data":"{\"type\":\"mouseMove\",\"x\":0,\"y\":0}"},{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#).unwrap();
                assert_eq!(speed.clone().publish(), 5.);
                let event = serde_json::json!([{
                    "timeTick":text,"data":r#"{"type":"mouseMove","x":99,"y":99}"#,
                }]);
                speed.observe(&event.to_string()).unwrap();
                assert_eq!(
                    speed.clone().publish(),
                    after_nonfinite,
                    "{product_id}: {text} after finite baseline",
                );
                assert_eq!(speed.last_tick.text.as_deref(), Some(text));
                speed
                    .observe(
                        r#"[{"timeTick":30000,"data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                    )
                    .unwrap();
                assert_eq!(
                    speed.clone().publish(),
                    after_first_finite,
                    "{product_id}: first finite tick after {text}",
                );
                speed
                    .observe(
                        r#"[{"timeTick":40000,"data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#,
                    )
                    .unwrap();
                assert_eq!(speed.publish(), 10., "{product_id}: recovered after {text}");
            }
        }
    }
    #[test]
    fn native_string_ticks_keep_the_callback_comparison_and_strict_zero_gate() {
        let mut speed = Speed::new(190).unwrap();
        speed.observe(r#"[{"timeTick":"9","data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"},{"timeTick":"10","data":"{\"type\":\"mouseMove\",\"x\":6,\"y\":8}"}]"#).unwrap();
        assert_eq!(speed.publish(), 0.);
        speed.observe(r#"[{"timeTick":"0","data":"{\"type\":\"mouseMove\",\"x\":10,\"y\":4}"},{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":12,\"y\":6}"}]"#).unwrap();
        assert_eq!(speed.publish(), 180f64.sqrt());
    }
    #[test]
    fn dynamic_speed_uses_absolute_axes_increasing_ticks_and_publication_reset() {
        let mut speed = Speed::new(190).unwrap();
        speed.observe(r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":90,\"y\":90}"},{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":-3,\"y\":4}"},{"timeTick":30000,"data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":-4}"}]"#).unwrap();
        assert_eq!(speed.publish(), 5.);
        speed
            .observe(r#"[{"timeTick":50000,"data":"{\"type\":\"mouseMove\",\"x\":80,\"y\":80}"}]"#)
            .unwrap();
        assert_eq!(speed.publish(), 0.);
    }
    #[test]
    fn dynamic_speed_rejects_partial_or_invalid_callback_without_losing_baseline() {
        let mut speed = Speed::new(190).unwrap();
        speed
            .observe(r#"[{"timeTick":10000,"data":"{\"type\":\"mouseMove\",\"x\":0,\"y\":0}"}]"#)
            .unwrap();
        assert!(
            speed
                .observe(r#"[{"data":"{\"type\":\"mouseMove\",\"x\":2,\"y\":1}"}]"#)
                .is_err()
        );
        speed
            .observe(r#"[{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#)
            .unwrap();
        assert_eq!(speed.publish(), 5.);
    }
}
