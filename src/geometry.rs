#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointI {
    pub x: i32,
    pub y: i32,
}

impl PointI {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RectI {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectI {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_points(a: PointI, b: PointI) -> Self {
        Self {
            left: a.x.min(b.x),
            top: a.y.min(b.y),
            right: a.x.max(b.x),
            bottom: a.y.max(b.y),
        }
    }

    pub const fn width(self) -> i32 {
        self.right - self.left
    }

    pub const fn height(self) -> i32 {
        self.bottom - self.top
    }

    pub const fn is_empty(self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }

    pub const fn translated(self, dx: i32, dy: i32) -> Self {
        Self {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right + dx,
            bottom: self.bottom + dy,
        }
    }

    pub fn clamp_origin_inside(self, bounds: Self, visible_margin: i32) -> PointI {
        let width = self.width();
        let height = self.height();
        let margin = visible_margin.max(1);

        let min_x = bounds.left - width + margin;
        let max_x = bounds.right - margin;
        let min_y = bounds.top - height + margin;
        let max_y = bounds.bottom - margin;

        PointI {
            x: clamp_even_if_inverted(self.left, min_x, max_x),
            y: clamp_even_if_inverted(self.top, min_y, max_y),
        }
    }
}

fn clamp_even_if_inverted(value: i32, minimum: i32, maximum: i32) -> i32 {
    if minimum <= maximum {
        value.clamp(minimum, maximum)
    } else {
        ((minimum as i64 + maximum as i64) / 2) as i32
    }
}

pub fn scaled_dimension(source: i32, scale: f64) -> i32 {
    ((source as f64 * scale).round() as i64).clamp(16, 32_767) as i32
}

pub fn zoom_around_point(
    old_rect: RectI,
    cursor: PointI,
    new_width: i32,
    new_height: i32,
) -> PointI {
    let old_width = old_rect.width().max(1) as f64;
    let old_height = old_rect.height().max(1) as f64;

    let ratio_x = ((cursor.x - old_rect.left) as f64 / old_width).clamp(0.0, 1.0);
    let ratio_y = ((cursor.y - old_rect.top) as f64 / old_height).clamp(0.0, 1.0);

    PointI {
        x: (cursor.x as f64 - ratio_x * new_width as f64).round() as i32,
        y: (cursor.y as f64 - ratio_y * new_height as f64).round() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_from_points_is_normalized() {
        assert_eq!(
            RectI::from_points(PointI::new(10, 20), PointI::new(-5, 50)),
            RectI::new(-5, 20, 10, 50)
        );
    }

    #[test]
    fn zoom_keeps_cursor_anchor() {
        let old = RectI::new(100, 100, 300, 200);
        let cursor = PointI::new(150, 125);
        let origin = zoom_around_point(old, cursor, 400, 200);
        assert_eq!(origin, PointI::new(50, 75));
    }

    #[test]
    fn inverted_clamp_range_does_not_panic() {
        let window = RectI::new(0, 0, 1, 1);
        let bounds = RectI::new(0, 0, 10, 10);
        assert_eq!(window.clamp_origin_inside(bounds, 32), PointI::new(4, 4));
    }

    #[test]
    fn clamp_keeps_a_visible_strip() {
        let window = RectI::new(-500, -500, -300, -300);
        let bounds = RectI::new(0, 0, 1920, 1080);
        assert_eq!(
            window.clamp_origin_inside(bounds, 32),
            PointI::new(-168, -168)
        );
    }
}
