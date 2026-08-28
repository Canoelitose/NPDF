//! The proof that an edit changed only what it was supposed to change.
//!
//! Render the page before and after, compare the two bitmaps, and report every
//! pixel that differs outside the edited region. Outside the edit the difference
//! has to be zero. This is the developer command asked for in the
//! specification and the safety net for every future text change.

use serde::{Deserialize, Serialize};

use super::RenderedPage;
use crate::error::{Error, Result};

/// A rectangle in device pixels, origin in the top left corner, which is how the
/// bitmap is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffReport {
    pub width: u32,
    pub height: u32,
    /// Pixels that differ anywhere on the page.
    pub changed_pixels: usize,
    /// Pixels that differ outside the region the edit was allowed to touch.
    /// This number has to be zero.
    pub changed_outside: usize,
    /// Largest difference of a single colour channel.
    pub max_channel_delta: u8,
    /// Smallest rectangle that holds every difference outside the region.
    pub outside_bounds: Option<PixelRect>,
    pub ok: bool,
}

/// Compare two renderings of the same page at the same scale.
///
/// `allowed` is the region an edit was expected to change. Pass `None` to demand
/// that the two renderings are identical everywhere.
///
/// `tolerance` is the per channel difference that still counts as equal. Use
/// zero for the strict check. A small value is useful when comparing across two
/// machines, where anti aliasing can differ by one step.
pub fn compare(
    before: &RenderedPage,
    after: &RenderedPage,
    allowed: Option<PixelRect>,
    tolerance: u8,
) -> Result<DiffReport> {
    if before.width != after.width || before.height != after.height {
        return Err(Error::InvalidArgument(format!(
            "the two renderings have different sizes: {}x{} and {}x{}",
            before.width, before.height, after.width, after.height
        )));
    }
    let expected = before.width as usize * before.height as usize * 4;
    if before.rgba.len() != expected || after.rgba.len() != expected {
        return Err(Error::InvalidArgument(
            "a rendering does not carry four bytes per pixel".into(),
        ));
    }

    let mut changed_pixels = 0usize;
    let mut changed_outside = 0usize;
    let mut max_delta = 0u8;
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for y in 0..before.height {
        for x in 0..before.width {
            let offset = ((y as usize * before.width as usize) + x as usize) * 4;
            let a = &before.rgba[offset..offset + 4];
            let b = &after.rgba[offset..offset + 4];

            let mut delta = 0u8;
            for channel in 0..4 {
                delta = delta.max(a[channel].abs_diff(b[channel]));
            }
            if delta <= tolerance {
                continue;
            }

            changed_pixels += 1;
            max_delta = max_delta.max(delta);

            let inside = allowed.map(|rect| rect.contains(x, y)).unwrap_or(false);
            if !inside {
                changed_outside += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    let outside_bounds = (changed_outside > 0).then(|| PixelRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    });

    Ok(DiffReport {
        width: before.width,
        height: before.height,
        changed_pixels,
        changed_outside,
        max_channel_delta: max_delta,
        outside_bounds,
        ok: changed_outside == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(width: u32, height: u32) -> RenderedPage {
        RenderedPage {
            page_index: 0,
            width,
            height,
            scale: 1.0,
            rgba: vec![255u8; (width * height * 4) as usize],
        }
    }

    fn set(page: &mut RenderedPage, x: u32, y: u32, value: [u8; 4]) {
        let offset = ((y as usize * page.width as usize) + x as usize) * 4;
        page.rgba[offset..offset + 4].copy_from_slice(&value);
    }

    #[test]
    fn identical_renderings_report_no_change() {
        let before = white(8, 8);
        let after = before.clone();
        let report = compare(&before, &after, None, 0).unwrap();
        assert!(report.ok);
        assert_eq!(report.changed_pixels, 0);
        assert_eq!(report.max_channel_delta, 0);
        assert!(report.outside_bounds.is_none());
    }

    #[test]
    fn a_change_inside_the_allowed_region_is_accepted() {
        let before = white(8, 8);
        let mut after = before.clone();
        set(&mut after, 3, 3, [0, 0, 0, 255]);
        let region = PixelRect {
            x: 2,
            y: 2,
            width: 3,
            height: 3,
        };
        let report = compare(&before, &after, Some(region), 0).unwrap();
        assert!(report.ok);
        assert_eq!(report.changed_pixels, 1);
        assert_eq!(report.changed_outside, 0);
        assert_eq!(report.max_channel_delta, 255);
    }

    #[test]
    fn a_change_outside_the_allowed_region_fails_and_is_located() {
        let before = white(8, 8);
        let mut after = before.clone();
        set(&mut after, 6, 7, [0, 0, 0, 255]);
        let region = PixelRect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        let report = compare(&before, &after, Some(region), 0).unwrap();
        assert!(!report.ok);
        assert_eq!(report.changed_outside, 1);
        assert_eq!(
            report.outside_bounds,
            Some(PixelRect {
                x: 6,
                y: 7,
                width: 1,
                height: 1
            })
        );
    }

    #[test]
    fn tolerance_absorbs_a_single_step_of_anti_aliasing() {
        let before = white(4, 4);
        let mut after = before.clone();
        set(&mut after, 1, 1, [254, 255, 255, 255]);
        assert!(!compare(&before, &after, None, 0).unwrap().ok);
        assert!(compare(&before, &after, None, 1).unwrap().ok);
    }

    #[test]
    fn different_sizes_are_rejected() {
        let error = compare(&white(4, 4), &white(4, 5), None, 0).unwrap_err();
        assert_eq!(error.code(), "invalid_argument");
    }
}
