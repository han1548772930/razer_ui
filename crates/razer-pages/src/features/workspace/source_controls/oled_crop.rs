//! Current product-691 CropperJS viewMode 0 geometry.
use serde::{Deserialize, Serialize};

pub(super) const WIDTH: f32 = 232.;
pub(super) const HEIGHT: f32 = 190.;
pub(super) const CROP_HEIGHT: f32 = 64.;
pub(super) const CROP_TOP: f32 = (HEIGHT - CROP_HEIGHT) / 2.;

// Any u32 image aspect at the 64px minimum fits below 2^38. This guard
// therefore accepts every valid initial canvas while rejecting huge saved
// floats and zoom values that would overflow subsequent layout arithmetic.
const MAX_CANVAS_EDGE: f32 = 1_099_511_627_776.; // 2^40 CSS pixels

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(super) struct CropCanvas {
    width: f32,
    height: f32,
    left: f32,
    top: f32,
}

impl CropCanvas {
    pub(super) fn new(width: f32, height: f32) -> Self {
        let aspect = width / height;
        // initCanvas uses contain for viewMode 0. limitCanvas then applies
        // minCanvasHeight 64 and derives minWidth from the image aspect.
        let height = (WIDTH / aspect).min(HEIGHT).max(CROP_HEIGHT);
        let width = height * aspect;
        Self {
            width,
            height,
            left: (WIDTH - width) / 2.,
            top: (HEIGHT - height) / 2.,
        }
    }

    pub(super) fn width(self) -> f32 {
        self.width
    }
    pub(super) fn height(self) -> f32 {
        self.height
    }
    pub(super) fn left(self) -> f32 {
        self.left
    }
    pub(super) fn top(self) -> f32 {
        self.top
    }

    pub(super) fn normalized(mut self) -> Option<Self> {
        if ![self.width, self.height, self.left, self.top]
            .into_iter()
            .all(f32::is_finite)
            || self.width <= 0.
            || self.height < CROP_HEIGHT
            || self.width > MAX_CANVAS_EDGE
            || self.height > MAX_CANVAS_EDGE
        {
            return None;
        }
        self.limit_position();
        Some(self)
    }

    fn limit_position(&mut self) {
        // viewMode 0 deliberately allows blank space in the crop box.
        self.left = self.left.clamp(-self.width, WIDTH);
        self.top = self.top.clamp(-self.height, HEIGHT);
    }

    pub(super) fn move_by(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.left += x;
            self.top += y;
            self.limit_position();
        }
    }

    pub(super) fn zoom_by(&mut self, delta: f32) {
        let factor = if delta < 0. { 1. / (1. - delta) } else { 1. + delta };
        let width = self.width * factor;
        let height = self.height * factor;
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || width > MAX_CANVAS_EDGE
            || height > MAX_CANVAS_EDGE
        {
            return;
        }
        // zoomTo without a pointer anchor keeps the canvas centre fixed.
        // renderCanvas restores oldLeft/oldTop when a size falls below its
        // minimum, then clamps the size. It does not recenter the minimum.
        if height >= CROP_HEIGHT {
            self.left -= (width - self.width) / 2.;
            self.top -= (height - self.height) / 2.;
            self.width = width;
            self.height = height;
        } else {
            self.width *= CROP_HEIGHT / self.height;
            self.height = CROP_HEIGHT;
        }
        self.limit_position();
    }
}
