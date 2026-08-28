//! Per page metadata the frontend needs to lay out the view.

use lopdf::{Object, ObjectId};
use serde::{Deserialize, Serialize};

use super::Document;
use crate::geom::Rect;

/// A serialisable object reference. `lopdf::ObjectId` is a tuple, which turns
/// into an unlabelled array in JSON and reads badly on the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRef {
    pub number: u32,
    pub generation: u16,
}

impl From<ObjectId> for ObjectRef {
    fn from(value: ObjectId) -> Self {
        Self {
            number: value.0,
            generation: value.1,
        }
    }
}

impl From<ObjectRef> for ObjectId {
    fn from(value: ObjectRef) -> Self {
        (value.number, value.generation)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub index: usize,
    pub object: ObjectRef,
    pub media_box: Rect,
    pub crop_box: Rect,
    /// Normalised to 0, 90, 180 or 270.
    pub rotation: i32,
    /// Visible width in points, after rotation.
    pub width_pt: f64,
    /// Visible height in points, after rotation.
    pub height_pt: f64,
    /// How many content streams the page is built from. A page can have several.
    pub content_stream_count: usize,
    pub annotation_count: usize,
}

/// A4 in points, the fallback when a page has no usable box at all.
const DEFAULT_BOX: Rect = Rect::new(0.0, 0.0, 595.276, 841.89);

impl PageInfo {
    pub(crate) fn read(doc: &Document, index: usize, id: ObjectId) -> Self {
        let media_box = read_box(doc, id, b"MediaBox").unwrap_or(DEFAULT_BOX);
        // A missing crop box means the crop box equals the media box.
        let crop_box = read_box(doc, id, b"CropBox").unwrap_or(media_box);
        let rotation = normalise_rotation(
            doc.inherited(id, b"Rotate")
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0),
        );

        let (mut width, mut height) = (crop_box.width(), crop_box.height());
        if rotation == 90 || rotation == 270 {
            std::mem::swap(&mut width, &mut height);
        }

        let content_stream_count = doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Contents").ok())
            .map(|contents| match doc.resolve(contents) {
                Ok(Object::Array(items)) => items.len(),
                Ok(_) => 1,
                Err(_) => 0,
            })
            .unwrap_or(0);

        let annotation_count = doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
            .and_then(|annots| doc.resolve(annots).ok())
            .and_then(|annots| annots.as_array().ok())
            .map(|a| a.len())
            .unwrap_or(0);

        Self {
            index,
            object: id.into(),
            media_box,
            crop_box,
            rotation,
            width_pt: width,
            height_pt: height,
            content_stream_count,
            annotation_count,
        }
    }
}

fn read_box(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Rect> {
    let object = doc.inherited(page_id, key)?;
    let array = object.as_array().ok()?;
    if array.len() != 4 {
        return None;
    }
    let mut values = [0.0f64; 4];
    for (slot, item) in values.iter_mut().zip(array) {
        let resolved = doc.resolve(item).ok()?;
        *slot = resolved.as_float().ok()? as f64;
    }
    let rect = Rect::new(values[0], values[1], values[2], values[3]).normalized();
    // A zero sized box is broken, fall back to the caller's default.
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    Some(rect)
}

/// PDF allows any multiple of 90, including negative values.
pub fn normalise_rotation(value: i64) -> i32 {
    let mut deg = value % 360;
    if deg < 0 {
        deg += 360;
    }
    // Round to the nearest quarter turn, broken files sometimes store 45.
    let quarter = ((deg as f64) / 90.0).round() as i64 % 4;
    (quarter * 90) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_is_normalised_into_quarter_turns() {
        assert_eq!(normalise_rotation(0), 0);
        assert_eq!(normalise_rotation(90), 90);
        assert_eq!(normalise_rotation(-90), 270);
        assert_eq!(normalise_rotation(450), 90);
        assert_eq!(normalise_rotation(-450), 270);
        assert_eq!(normalise_rotation(360), 0);
    }
}
