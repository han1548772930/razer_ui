//! Current seven-product dynamic-sensitivity graph mathematics, without I/O.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}

pub(super) const CLASSIC: [Point; 5] = [
    Point { x: 0., y: 1. },
    Point { x: 20., y: 1.1 },
    Point { x: 40., y: 1.2 },
    Point { x: 70., y: 1.35 },
    Point { x: 100., y: 1.5 },
];
pub(super) const NATURAL: [Point; 5] = [
    Point { x: 0., y: 1. },
    Point { x: 11., y: 1.45 },
    Point { x: 20., y: 1.5 },
    Point { x: 35., y: 1.5 },
    Point { x: 100., y: 1.5 },
];
pub(super) const JUMP: [Point; 5] = [
    Point { x: 0., y: 1. },
    Point { x: 9., y: 1. },
    Point { x: 21., y: 1.5 },
    Point { x: 24., y: 1.5 },
    Point { x: 100., y: 1.5 },
];
pub(super) fn preset(mode: u32) -> Option<&'static [Point; 5]> {
    match mode {
        0 => Some(&CLASSIC),
        1 => Some(&NATURAL),
        2 => Some(&JUMP),
        _ => None,
    }
}

/// Original kS/xS (190), vt/Ct (226): shape-preserving cubic Hermite.
/// The UI supplies ordered points. Invalid observed coordinates are rejected,
/// rather than invoking the vendor JS's in-place sort on malformed data.
pub(super) struct Curve {
    x: Vec<f64>,
    coefficients: Vec<[f64; 4]>,
}
impl Curve {
    pub fn new(points: &[Point]) -> Option<Self> {
        if points.len() < 3
            || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
            || points.windows(2).any(|p| p[1].x <= p[0].x)
        {
            return None;
        }
        let n = points.len();
        let gaps: Vec<_> = points.windows(2).map(|p| p[1].x - p[0].x).collect();
        let secants: Vec<_> = points
            .windows(2)
            .zip(&gaps)
            .map(|(p, h)| (p[1].y - p[0].y) / h)
            .collect();
        let mut derivatives = vec![0.; n];
        for i in 1..n - 1 {
            let previous = secants[i - 1];
            let next = secants[i];
            if previous.abs() < f64::EPSILON
                || next.abs() < f64::EPSILON
                || previous.signum() != next.signum()
            {
                continue;
            }
            let w1 = 2. * gaps[i] + gaps[i - 1];
            let w2 = gaps[i] + 2. * gaps[i - 1];
            derivatives[i] = (w1 + w2) / (w1 / previous + w2 / next);
        }
        derivatives[0] = endpoint(gaps[0], gaps[1], secants[0], secants[1]);
        derivatives[n - 1] = endpoint(gaps[n - 2], gaps[n - 3], secants[n - 2], secants[n - 3]);
        let coefficients = (0..n - 1)
            .map(|i| {
                let h = gaps[i];
                let delta = points[i + 1].y - points[i].y;
                [
                    points[i].y,
                    derivatives[i],
                    (3. * delta / h - 2. * derivatives[i] - derivatives[i + 1]) / h,
                    (-2. * delta / h + derivatives[i] + derivatives[i + 1]) / (h * h),
                ]
            })
            .collect();
        Some(Self {
            x: points.iter().map(|p| p.x).collect(),
            coefficients,
        })
    }
    pub fn interpolate(&self, x: f64) -> f64 {
        let index = self
            .x
            .partition_point(|value| *value <= x)
            .saturating_sub(1)
            .min(self.coefficients.len() - 1);
        let delta = x - self.x[index];
        let c = self.coefficients[index];
        c[0] + delta * (c[1] + delta * (c[2] + delta * c[3]))
    }
}
fn endpoint(h0: f64, h1: f64, d0: f64, d1: f64) -> f64 {
    let d = ((2. * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
    // JS Math.sign(0) == 0, unlike Rust's signum(+0).
    let sign = |v: f64| if v == 0. { 0. } else { v.signum() };
    if sign(d) != sign(d0) {
        0.
    } else if sign(d0) != sign(d1) && d.abs() > 3. * d0.abs() {
        3. * d0
    } else {
        d
    }
}

/// Original k/Y: first rounded-x hit in 1000 samples, carrying the last
/// acceleration through the remaining 250 entries; not a direct sample at x.
pub(super) fn acceleration_table(points: &[Point]) -> Option<Vec<f64>> {
    let curve = Curve::new(points)?;
    let last_x = points.last()?.x;
    if last_x <= 0. {
        return None;
    }
    let samples: Vec<_> = (0..1000)
        .map(|index| {
            let x = index as f64 * last_x / 999.;
            Point {
                x,
                y: curve.interpolate(x),
            }
        })
        .collect();
    let mut carry: f64 = 0.;
    Some(
        (1..=250)
            .map(|target| {
                let target = target as f64;
                for sample in &samples {
                    // Coordinates are nonnegative, so floor(x+0.5) is Math.round(x).
                    if (sample.x + 0.5).floor() == target {
                        carry = sample.y;
                        break;
                    }
                    if sample.x > target {
                        break;
                    }
                }
                carry.max(0.1)
            })
            .collect(),
    )
}

/// Keep the source's loop order and asymmetric neighbour propagation.
pub(super) fn drag(points: &mut [Point], selected: usize, point: Point, max_x: f64, max_y: f64) {
    if selected >= points.len() || !point.x.is_finite() || !point.y.is_finite() {
        return;
    }
    points[selected] = point;
    for a in 0..points.len() {
        if a == 0 {
            points[a].x = 0.;
        } else if points[a].x <= 2. {
            points[a].x = 2.;
        }
        if points[selected].y > max_y {
            points[selected].y = max_y;
        }
        if points[a].x >= max_x {
            points[a].x = max_x;
        }
        if points[a].y <= 0.1 {
            points[a].y = 0.1;
        }
        if a < selected && a > 0 {
            for index in 1..selected {
                if points[index + 1].x - points[index].x < 2. && points[index].x > 2. * index as f64
                {
                    points[index].x = points[index + 1].x - 2.;
                }
                if points[selected].x < 2. * selected as f64 {
                    points[selected].x = 2. * selected as f64;
                }
            }
        }
        if a > selected && selected > 0 {
            for index in (selected + 1..points.len()).rev() {
                if points[index].x - points[index - 1].x < 2. {
                    points[index].x = (points[index - 1].x + 2.).min(max_x);
                }
                let limit = max_x - 2. * (points.len() - 1 - selected) as f64;
                if points[selected].x > limit {
                    points[selected].x = limit;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_presets_interpolate_knots_without_overshooting_plateaus() {
        for points in [&CLASSIC, &NATURAL, &JUMP] {
            let curve = Curve::new(points).unwrap();
            for point in points {
                assert!((curve.interpolate(point.x) - point.y).abs() < 1e-12);
            }
            for x in 0..1000 {
                assert!((1. - 1e-12..=1.5 + 1e-12).contains(&curve.interpolate(x as f64 / 10.)));
            }
        }
        assert_eq!(Curve::new(&JUMP).unwrap().interpolate(8.), 1.);
        assert_eq!(Curve::new(&NATURAL).unwrap().interpolate(95.), 1.5);
    }
    #[test]
    fn rounded_sampling_and_last_value_carry_are_preserved() {
        let table = acceleration_table(&CLASSIC).unwrap();
        assert_eq!(table.len(), 250);
        // First hit for rounded x=1 is sample 5, not a direct x=1 query.
        assert!((table[0] - (1. + 0.005 * 500. / 999.)).abs() < 1e-12);
        assert!(table[99..].windows(2).all(|v| v[0] == v[1]));
    }
    #[test]
    fn drag_fixes_origin_and_preserves_source_neighbour_push() {
        let mut points = CLASSIC;
        drag(&mut points, 0, Point { x: 30., y: -1. }, 105., 1.5);
        assert_eq!(points[0], Point { x: 0., y: 0.1 });
        drag(&mut points, 2, Point { x: 103., y: 5. }, 105., 1.5);
        assert_eq!(points[2], Point { x: 101., y: 1.5 });
        assert_eq!(points[3].x, 103.);
        assert_eq!(points[4].x, 105.);
        assert!(Curve::new(&[Point { x: 0., y: 1. }; 5]).is_none());
    }
}
