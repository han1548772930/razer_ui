//! Current movement callback arithmetic. Publication resets the tick baseline,
//! while the page owns its separate 150 ms visible-speed expiry.
use anyhow::{Context as _, ensure};
use serde::Deserialize;

#[derive(Default)]
pub struct Speed {
    last_tick: f64,
    speed: f64,
}

impl Speed {
    pub fn observe(&mut self, json_event: &str) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Tick {
            Number(f64),
            Text(String),
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            data: String,
            time_tick: Option<Tick>,
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
        let mut last = self.last_tick;
        for row in rows {
            let Data::Move { x: dx, y: dy } = serde_json::from_str(&row.data)? else {
                continue;
            };
            let tick = match row.time_tick.context("Dynamic movement omits timeTick")? {
                Tick::Number(value) => value,
                Tick::Text(value) => value
                    .parse::<f64>()
                    .context("Dynamic movement has invalid timeTick")?,
            };
            ensure!(
                tick.is_finite() && tick >= 0. && dx.is_finite() && dy.is_finite(),
                "Dynamic movement contains nonfinite or negative timing data"
            );
            if last != 0. && tick > last {
                x += dx.abs();
                y += dy.abs();
                ticks += tick - last;
            }
            last = tick;
        }
        if ticks > 0. {
            self.speed = x.hypot(y) / (ticks / 1e4);
            ensure!(self.speed.is_finite(), "Dynamic movement speed overflowed");
        }
        self.last_tick = last;
        Ok(())
    }

    pub fn publish(&mut self) -> f64 {
        let speed = self.speed;
        *self = Self::default();
        speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dynamic_speed_uses_absolute_axes_increasing_ticks_and_publication_reset() {
        let mut speed = Speed::default();
        speed.observe(r#"[{"timeTick":"10000","data":"{\"type\":\"mouseMove\",\"x\":90,\"y\":90}"},{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":-3,\"y\":4}"},{"timeTick":30000,"data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":-4}"}]"#).unwrap();
        assert_eq!(speed.publish(), 5.);
        speed
            .observe(r#"[{"timeTick":50000,"data":"{\"type\":\"mouseMove\",\"x\":80,\"y\":80}"}]"#)
            .unwrap();
        assert_eq!(speed.publish(), 0.);
    }
    #[test]
    fn dynamic_speed_rejects_partial_or_invalid_callback_without_losing_baseline() {
        let mut speed = Speed::default();
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
