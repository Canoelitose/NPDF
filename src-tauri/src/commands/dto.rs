//! Data that crosses the bridge to the frontend.

use npdf_core::render::RenderedPage;
use npdf_core::save::SaveReport;
use serde::Serialize;

/// Errors reach the frontend as a code plus a message. The code is stable, the
/// message is for the log, and the German wording the user sees is picked by the
/// frontend from the code.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<npdf_core::Error> for CommandError {
    fn from(value: npdf_core::Error) -> Self {
        Self {
            code: value.code().to_string(),
            message: value.to_string(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

/// Length of the fixed header in front of the pixel data.
pub const RENDER_HEADER_BYTES: usize = 20;

/// Pack a rendered page into one buffer.
///
/// Layout, all little endian:
///
/// | offset | type | meaning              |
/// |--------|------|----------------------|
/// | 0      | u32  | width in pixels      |
/// | 4      | u32  | height in pixels     |
/// | 8      | f32  | scale that was used  |
/// | 12     | u32  | page index           |
/// | 16     | u32  | length of the pixels |
/// | 20     | ...  | RGBA, eight bits per channel, top row first |
pub fn encode_rendered_page(page: &RenderedPage) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(RENDER_HEADER_BYTES + page.rgba.len());
    buffer.extend_from_slice(&page.width.to_le_bytes());
    buffer.extend_from_slice(&page.height.to_le_bytes());
    buffer.extend_from_slice(&(page.scale as f32).to_le_bytes());
    buffer.extend_from_slice(&(page.page_index as u32).to_le_bytes());
    buffer.extend_from_slice(&(page.rgba.len() as u32).to_le_bytes());
    buffer.extend_from_slice(&page.rgba);
    buffer
}

/// Pack a saved document together with its report.
///
/// | offset | type | meaning                              |
/// |--------|------|--------------------------------------|
/// | 0      | u32  | length of the report, as UTF-8 JSON  |
/// | 4      | ...  | the report                           |
/// | ...    | ...  | the PDF bytes                        |
pub fn encode_saved_document(bytes: &[u8], report: &SaveReport) -> Result<Vec<u8>, CommandError> {
    let json = serde_json::to_vec(report).map_err(|e| CommandError {
        code: "save".to_string(),
        message: format!("the save report could not be serialised: {e}"),
    })?;
    let mut buffer = Vec::with_capacity(4 + json.len() + bytes.len());
    buffer.extend_from_slice(&(json.len() as u32).to_le_bytes());
    buffer.extend_from_slice(&json);
    buffer.extend_from_slice(bytes);
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendered_page_round_trips_through_the_header() {
        let page = RenderedPage {
            page_index: 3,
            width: 2,
            height: 1,
            scale: 1.5,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let encoded = encode_rendered_page(&page);
        assert_eq!(encoded.len(), RENDER_HEADER_BYTES + 8);
        assert_eq!(u32::from_le_bytes(encoded[0..4].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(encoded[4..8].try_into().unwrap()), 1);
        assert_eq!(f32::from_le_bytes(encoded[8..12].try_into().unwrap()), 1.5);
        assert_eq!(u32::from_le_bytes(encoded[12..16].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(encoded[16..20].try_into().unwrap()), 8);
        assert_eq!(&encoded[RENDER_HEADER_BYTES..], page.rgba.as_slice());
    }

    #[test]
    fn an_error_keeps_its_code() {
        let error: CommandError = npdf_core::Error::PasswordRequired.into();
        assert_eq!(error.code, "password_required");
    }
}
