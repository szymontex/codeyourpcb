//! Geometry types for PCB layout.
//!
//! This module provides basic geometric primitives for representing
//! shapes, bounds, and regions on a PCB.

use crate::coords::{Nm, Point};
use serde::{Deserialize, Serialize};

/// An axis-aligned bounding box (AABB) in nanometers.
///
/// Represents a rectangular region defined by its minimum and maximum corners.
/// The min corner has the smallest x and y values, max has the largest.
///
/// # Invariant
///
/// `min.x <= max.x` and `min.y <= max.y`. Use `from_points` to ensure this.
///
/// # Examples
///
/// ```
/// use cypcb_core::{Rect, Point, Nm};
///
/// let rect = Rect::from_points(
///     Point::from_mm(0.0, 0.0),
///     Point::from_mm(10.0, 20.0),
/// );
///
/// assert_eq!(rect.width(), Nm::from_mm(10.0));
/// assert_eq!(rect.height(), Nm::from_mm(20.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    /// Minimum corner (smallest x and y).
    pub min: Point,
    /// Maximum corner (largest x and y).
    pub max: Point,
}

impl Rect {
    /// Create a new rectangle from min and max corners.
    ///
    /// # Note
    ///
    /// Assumes min <= max. Use `from_points` if this is not guaranteed.
    #[inline]
    pub const fn new(min: Point, max: Point) -> Self {
        Rect { min, max }
    }

    /// Create a rectangle from two arbitrary corner points.
    ///
    /// The points are normalized so that min is truly the minimum
    /// and max is truly the maximum corner.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// // Points can be in any order
    /// let r1 = Rect::from_points(
    ///     Point::from_mm(10.0, 20.0),
    ///     Point::from_mm(0.0, 0.0),
    /// );
    /// let r2 = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// assert_eq!(r1, r2);
    /// ```
    pub fn from_points(p1: Point, p2: Point) -> Self {
        Rect {
            min: Point {
                x: Nm(p1.x.0.min(p2.x.0)),
                y: Nm(p1.y.0.min(p2.y.0)),
            },
            max: Point {
                x: Nm(p1.x.0.max(p2.x.0)),
                y: Nm(p1.y.0.max(p2.y.0)),
            },
        }
    }

    /// Create a rectangle from a center point and half-widths.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point, Nm};
    ///
    /// let rect = Rect::from_center_half_size(
    ///     Point::from_mm(10.0, 10.0),
    ///     Nm::from_mm(5.0),
    ///     Nm::from_mm(3.0),
    /// );
    ///
    /// assert_eq!(rect.min, Point::from_mm(5.0, 7.0));
    /// assert_eq!(rect.max, Point::from_mm(15.0, 13.0));
    /// ```
    pub fn from_center_half_size(center: Point, half_width: Nm, half_height: Nm) -> Self {
        Rect {
            min: Point {
                x: Nm(center.x.0 - half_width.0),
                y: Nm(center.y.0 - half_height.0),
            },
            max: Point {
                x: Nm(center.x.0 + half_width.0),
                y: Nm(center.y.0 + half_height.0),
            },
        }
    }

    /// Create a rectangle from a center point and full size (width, height).
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point, Nm};
    ///
    /// let rect = Rect::from_center_size(
    ///     Point::from_mm(10.0, 10.0),
    ///     (Nm::from_mm(10.0), Nm::from_mm(6.0)),
    /// );
    ///
    /// assert_eq!(rect.min, Point::from_mm(5.0, 7.0));
    /// assert_eq!(rect.max, Point::from_mm(15.0, 13.0));
    /// ```
    pub fn from_center_size(center: Point, size: (Nm, Nm)) -> Self {
        let half_width = Nm(size.0 .0 / 2);
        let half_height = Nm(size.1 .0 / 2);
        Self::from_center_half_size(center, half_width, half_height)
    }

    /// Create a rectangle from origin and size.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point, Nm};
    ///
    /// let rect = Rect::from_origin_size(
    ///     Point::from_mm(5.0, 10.0),
    ///     Nm::from_mm(20.0),
    ///     Nm::from_mm(15.0),
    /// );
    ///
    /// assert_eq!(rect.min, Point::from_mm(5.0, 10.0));
    /// assert_eq!(rect.max, Point::from_mm(25.0, 25.0));
    /// ```
    pub fn from_origin_size(origin: Point, width: Nm, height: Nm) -> Self {
        Rect {
            min: origin,
            max: Point {
                x: Nm(origin.x.0 + width.0),
                y: Nm(origin.y.0 + height.0),
            },
        }
    }

    /// Get the width of the rectangle.
    #[inline]
    pub fn width(&self) -> Nm {
        Nm(self.max.x.0 - self.min.x.0)
    }

    /// Get the height of the rectangle.
    #[inline]
    pub fn height(&self) -> Nm {
        Nm(self.max.y.0 - self.min.y.0)
    }

    /// Get the center point of the rectangle.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// let rect = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 20.0),
    /// );
    ///
    /// assert_eq!(rect.center(), Point::from_mm(5.0, 10.0));
    /// ```
    pub fn center(&self) -> Point {
        Point {
            x: Nm((self.min.x.0 + self.max.x.0) / 2),
            y: Nm((self.min.y.0 + self.max.y.0) / 2),
        }
    }

    /// Get the area of the rectangle in square nanometers.
    ///
    /// Returns as i128 to avoid overflow with large rectangles.
    #[inline]
    pub fn area(&self) -> i128 {
        self.width().0 as i128 * self.height().0 as i128
    }

    /// Check if a point is contained within this rectangle.
    ///
    /// Points on the boundary are considered contained.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// let rect = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 10.0),
    /// );
    ///
    /// assert!(rect.contains(Point::from_mm(5.0, 5.0)));
    /// assert!(rect.contains(Point::from_mm(0.0, 0.0)));  // On boundary
    /// assert!(!rect.contains(Point::from_mm(15.0, 5.0)));
    /// ```
    #[inline]
    pub fn contains(&self, p: Point) -> bool {
        p.x.0 >= self.min.x.0
            && p.x.0 <= self.max.x.0
            && p.y.0 >= self.min.y.0
            && p.y.0 <= self.max.y.0
    }

    /// Check if another rectangle is fully contained within this one.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// let outer = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(20.0, 20.0),
    /// );
    /// let inner = Rect::from_points(
    ///     Point::from_mm(5.0, 5.0),
    ///     Point::from_mm(15.0, 15.0),
    /// );
    ///
    /// assert!(outer.contains_rect(&inner));
    /// assert!(!inner.contains_rect(&outer));
    /// ```
    #[inline]
    pub fn contains_rect(&self, other: &Rect) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    /// Check if this rectangle intersects with another.
    ///
    /// Two rectangles intersect if they share any area, including
    /// just touching at an edge or corner.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// let r1 = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 10.0),
    /// );
    /// let r2 = Rect::from_points(
    ///     Point::from_mm(5.0, 5.0),
    ///     Point::from_mm(15.0, 15.0),
    /// );
    /// let r3 = Rect::from_points(
    ///     Point::from_mm(20.0, 20.0),
    ///     Point::from_mm(30.0, 30.0),
    /// );
    ///
    /// assert!(r1.intersects(&r2));
    /// assert!(!r1.intersects(&r3));
    /// ```
    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x.0 <= other.max.x.0
            && self.max.x.0 >= other.min.x.0
            && self.min.y.0 <= other.max.y.0
            && self.max.y.0 >= other.min.y.0
    }

    /// Compute the intersection of two rectangles.
    ///
    /// Returns `None` if the rectangles don't overlap.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// let r1 = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 10.0),
    /// );
    /// let r2 = Rect::from_points(
    ///     Point::from_mm(5.0, 5.0),
    ///     Point::from_mm(15.0, 15.0),
    /// );
    ///
    /// let intersection = r1.intersection(&r2).unwrap();
    /// assert_eq!(intersection.min, Point::from_mm(5.0, 5.0));
    /// assert_eq!(intersection.max, Point::from_mm(10.0, 10.0));
    /// ```
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }

        Some(Rect {
            min: Point {
                x: Nm(self.min.x.0.max(other.min.x.0)),
                y: Nm(self.min.y.0.max(other.min.y.0)),
            },
            max: Point {
                x: Nm(self.max.x.0.min(other.max.x.0)),
                y: Nm(self.max.y.0.min(other.max.y.0)),
            },
        })
    }

    /// Compute the bounding box that contains both rectangles.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point};
    ///
    /// let r1 = Rect::from_points(
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 10.0),
    /// );
    /// let r2 = Rect::from_points(
    ///     Point::from_mm(5.0, 5.0),
    ///     Point::from_mm(20.0, 15.0),
    /// );
    ///
    /// let union = r1.union(&r2);
    /// assert_eq!(union.min, Point::from_mm(0.0, 0.0));
    /// assert_eq!(union.max, Point::from_mm(20.0, 15.0));
    /// ```
    pub fn union(&self, other: &Rect) -> Rect {
        Rect {
            min: Point {
                x: Nm(self.min.x.0.min(other.min.x.0)),
                y: Nm(self.min.y.0.min(other.min.y.0)),
            },
            max: Point {
                x: Nm(self.max.x.0.max(other.max.x.0)),
                y: Nm(self.max.y.0.max(other.max.y.0)),
            },
        }
    }

    /// Expand the rectangle by a uniform amount in all directions.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Rect, Point, Nm};
    ///
    /// let rect = Rect::from_points(
    ///     Point::from_mm(5.0, 5.0),
    ///     Point::from_mm(10.0, 10.0),
    /// );
    /// let expanded = rect.expand(Nm::from_mm(2.0));
    ///
    /// assert_eq!(expanded.min, Point::from_mm(3.0, 3.0));
    /// assert_eq!(expanded.max, Point::from_mm(12.0, 12.0));
    /// ```
    pub fn expand(&self, amount: Nm) -> Rect {
        Rect {
            min: Point {
                x: Nm(self.min.x.0 - amount.0),
                y: Nm(self.min.y.0 - amount.0),
            },
            max: Point {
                x: Nm(self.max.x.0 + amount.0),
                y: Nm(self.max.y.0 + amount.0),
            },
        }
    }

    /// Shrink the rectangle by a uniform amount in all directions.
    ///
    /// If the amount is greater than half the width or height,
    /// the rectangle collapses to its center point.
    pub fn shrink(&self, amount: Nm) -> Rect {
        let center = self.center();
        let half_width = (self.width().0 / 2 - amount.0).max(0);
        let half_height = (self.height().0 / 2 - amount.0).max(0);

        Rect {
            min: Point {
                x: Nm(center.x.0 - half_width),
                y: Nm(center.y.0 - half_height),
            },
            max: Point {
                x: Nm(center.x.0 + half_width),
                y: Nm(center.y.0 + half_height),
            },
        }
    }

    /// Get the four corner points of the rectangle.
    ///
    /// Returns points in counter-clockwise order starting from min.
    pub fn corners(&self) -> [Point; 4] {
        [
            self.min,
            Point::new(self.max.x, self.min.y),
            self.max,
            Point::new(self.min.x, self.max.y),
        ]
    }

    /// Check if the rectangle is empty (zero area).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.min.x.0 >= self.max.x.0 || self.min.y.0 >= self.max.y.0
    }
}

impl Default for Rect {
    fn default() -> Self {
        Rect {
            min: Point::ORIGIN,
            max: Point::ORIGIN,
        }
    }
}

impl std::fmt::Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{} -> {}]", self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_from_points_normalizes() {
        let r1 = Rect::from_points(
            Point::from_mm(10.0, 20.0),
            Point::from_mm(0.0, 0.0),
        );
        let r2 = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 20.0),
        );

        assert_eq!(r1.min, r2.min);
        assert_eq!(r1.max, r2.max);
    }

    #[test]
    fn test_rect_dimensions() {
        let rect = Rect::from_points(
            Point::from_mm(5.0, 10.0),
            Point::from_mm(25.0, 40.0),
        );

        assert_eq!(rect.width(), Nm::from_mm(20.0));
        assert_eq!(rect.height(), Nm::from_mm(30.0));
    }

    #[test]
    fn test_rect_center() {
        let rect = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 20.0),
        );

        assert_eq!(rect.center(), Point::from_mm(5.0, 10.0));
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 10.0),
        );

        // Inside
        assert!(rect.contains(Point::from_mm(5.0, 5.0)));

        // On boundary
        assert!(rect.contains(Point::from_mm(0.0, 0.0)));
        assert!(rect.contains(Point::from_mm(10.0, 10.0)));
        assert!(rect.contains(Point::from_mm(5.0, 0.0)));

        // Outside
        assert!(!rect.contains(Point::from_mm(-1.0, 5.0)));
        assert!(!rect.contains(Point::from_mm(11.0, 5.0)));
        assert!(!rect.contains(Point::from_mm(5.0, -1.0)));
        assert!(!rect.contains(Point::from_mm(5.0, 11.0)));
    }

    #[test]
    fn test_rect_intersects() {
        let r1 = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 10.0),
        );

        // Overlapping
        let r2 = Rect::from_points(
            Point::from_mm(5.0, 5.0),
            Point::from_mm(15.0, 15.0),
        );
        assert!(r1.intersects(&r2));
        assert!(r2.intersects(&r1));

        // Touching edge
        let r3 = Rect::from_points(
            Point::from_mm(10.0, 0.0),
            Point::from_mm(20.0, 10.0),
        );
        assert!(r1.intersects(&r3));

        // Completely separate
        let r4 = Rect::from_points(
            Point::from_mm(20.0, 20.0),
            Point::from_mm(30.0, 30.0),
        );
        assert!(!r1.intersects(&r4));
    }

    #[test]
    fn test_rect_intersection() {
        let r1 = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 10.0),
        );
        let r2 = Rect::from_points(
            Point::from_mm(5.0, 5.0),
            Point::from_mm(15.0, 15.0),
        );

        let intersection = r1.intersection(&r2).unwrap();
        assert_eq!(intersection.min, Point::from_mm(5.0, 5.0));
        assert_eq!(intersection.max, Point::from_mm(10.0, 10.0));
    }

    #[test]
    fn test_rect_intersection_none() {
        let r1 = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 10.0),
        );
        let r2 = Rect::from_points(
            Point::from_mm(20.0, 20.0),
            Point::from_mm(30.0, 30.0),
        );

        assert!(r1.intersection(&r2).is_none());
    }

    #[test]
    fn test_rect_union() {
        let r1 = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 10.0),
        );
        let r2 = Rect::from_points(
            Point::from_mm(5.0, 5.0),
            Point::from_mm(20.0, 15.0),
        );

        let union = r1.union(&r2);
        assert_eq!(union.min, Point::from_mm(0.0, 0.0));
        assert_eq!(union.max, Point::from_mm(20.0, 15.0));
    }

    #[test]
    fn test_rect_expand() {
        let rect = Rect::from_points(
            Point::from_mm(5.0, 5.0),
            Point::from_mm(10.0, 10.0),
        );
        let expanded = rect.expand(Nm::from_mm(2.0));

        assert_eq!(expanded.min, Point::from_mm(3.0, 3.0));
        assert_eq!(expanded.max, Point::from_mm(12.0, 12.0));
    }

    #[test]
    fn test_rect_contains_rect() {
        let outer = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(20.0, 20.0),
        );
        let inner = Rect::from_points(
            Point::from_mm(5.0, 5.0),
            Point::from_mm(15.0, 15.0),
        );
        let partial = Rect::from_points(
            Point::from_mm(15.0, 15.0),
            Point::from_mm(25.0, 25.0),
        );

        assert!(outer.contains_rect(&inner));
        assert!(!inner.contains_rect(&outer));
        assert!(!outer.contains_rect(&partial));
    }

    #[test]
    fn test_rect_area() {
        let rect = Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(10.0, 5.0),
        );
        // 10mm * 5mm = 50 mm^2 = 50 * 10^12 nm^2
        assert_eq!(rect.area(), 50_000_000_000_000i128);
    }
}
