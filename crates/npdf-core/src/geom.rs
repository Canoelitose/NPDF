//! Small geometry helpers shared by extraction, editing and rendering.
//!
//! Everything here works in PDF user space: the origin sits in the lower left
//! corner, y grows upwards, one unit is 1/72 inch.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A PDF transformation matrix `[a b c d e f]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Matrix {
    fn default() -> Self {
        Matrix::IDENTITY
    }
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    pub const fn translate(tx: f64, ty: f64) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    pub const fn scale(sx: f64, sy: f64) -> Self {
        Self::new(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    pub fn rotate_degrees(deg: f64) -> Self {
        let (sin, cos) = deg.to_radians().sin_cos();
        Self::new(cos, sin, -sin, cos, 0.0, 0.0)
    }

    /// `self` applied first, then `other`. This is the order the PDF operators
    /// use: a `cm` operator premultiplies the current transformation matrix.
    pub fn then(&self, other: &Matrix) -> Matrix {
        Matrix {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    pub fn apply(&self, p: Point) -> Point {
        Point {
            x: self.a * p.x + self.c * p.y + self.e,
            y: self.b * p.x + self.d * p.y + self.f,
        }
    }

    /// Length of the transformed unit vector along x. Used to turn a font size
    /// in text space into a size on the page.
    pub fn x_scale(&self) -> f64 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    pub fn y_scale(&self) -> f64 {
        (self.c * self.c + self.d * self.d).sqrt()
    }

    pub fn rotation_degrees(&self) -> f64 {
        self.b.atan2(self.a).to_degrees()
    }

    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    pub fn invert(&self) -> Option<Matrix> {
        let det = self.determinant();
        if det.abs() < f64::EPSILON {
            return None;
        }
        let inv = 1.0 / det;
        Some(Matrix {
            a: self.d * inv,
            b: -self.b * inv,
            c: -self.c * inv,
            d: self.a * inv,
            e: (self.c * self.f - self.d * self.e) * inv,
            f: (self.b * self.e - self.a * self.f) * inv,
        })
    }

    pub fn to_array(self) -> [f64; 6] {
        [self.a, self.b, self.c, self.d, self.e, self.f]
    }

    pub fn from_array(v: [f64; 6]) -> Self {
        Self::new(v[0], v[1], v[2], v[3], v[4], v[5])
    }
}

/// An axis aligned rectangle. `x0`/`y0` is always the lower left corner after
/// [`Rect::normalized`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    };

    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn normalized(self) -> Self {
        Self {
            x0: self.x0.min(self.x1),
            y0: self.y0.min(self.y1),
            x1: self.x0.max(self.x1),
            y1: self.y0.max(self.y1),
        }
    }

    pub fn width(&self) -> f64 {
        (self.x1 - self.x0).abs()
    }

    pub fn height(&self) -> f64 {
        (self.y1 - self.y0).abs()
    }

    pub fn contains(&self, p: Point) -> bool {
        let r = self.normalized();
        p.x >= r.x0 && p.x <= r.x1 && p.y >= r.y0 && p.y <= r.y1
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let a = self.normalized();
        let b = other.normalized();
        Rect {
            x0: a.x0.min(b.x0),
            y0: a.y0.min(b.y0),
            x1: a.x1.max(b.x1),
            y1: a.y1.max(b.y1),
        }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        let a = self.normalized();
        let b = other.normalized();
        a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
    }

    /// Grow the rectangle by `amount` on every side.
    pub fn inflate(&self, amount: f64) -> Rect {
        let r = self.normalized();
        Rect {
            x0: r.x0 - amount,
            y0: r.y0 - amount,
            x1: r.x1 + amount,
            y1: r.y1 + amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_then_scale_matches_manual_multiplication() {
        let m = Matrix::translate(10.0, 20.0).then(&Matrix::scale(2.0, 3.0));
        assert_eq!(m, Matrix::new(2.0, 0.0, 0.0, 3.0, 20.0, 60.0));
    }

    #[test]
    fn identity_is_neutral() {
        let m = Matrix::new(1.5, 0.25, -0.5, 2.0, 7.0, -3.0);
        assert_eq!(m.then(&Matrix::IDENTITY), m);
        assert_eq!(Matrix::IDENTITY.then(&m), m);
    }

    #[test]
    fn inverse_round_trips_a_point() {
        let m = Matrix::rotate_degrees(30.0).then(&Matrix::translate(5.0, -2.0));
        let inv = m.invert().expect("matrix is invertible");
        let p = Point::new(3.0, 4.0);
        let back = inv.apply(m.apply(p));
        assert!((back.x - p.x).abs() < 1e-9, "x was {}", back.x);
        assert!((back.y - p.y).abs() < 1e-9, "y was {}", back.y);
    }

    #[test]
    fn scale_factors_survive_rotation() {
        let m = Matrix::scale(4.0, 4.0).then(&Matrix::rotate_degrees(45.0));
        assert!((m.x_scale() - 4.0).abs() < 1e-9);
        assert!((m.y_scale() - 4.0).abs() < 1e-9);
        assert!((m.rotation_degrees() - 45.0).abs() < 1e-9);
    }

    #[test]
    fn rect_union_and_contains() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, -5.0, 20.0, 5.0);
        assert_eq!(a.union(&b), Rect::new(0.0, -5.0, 20.0, 10.0));
        assert!(a.contains(Point::new(1.0, 1.0)));
        assert!(!a.contains(Point::new(11.0, 1.0)));
        assert!(a.intersects(&b));
    }
}
