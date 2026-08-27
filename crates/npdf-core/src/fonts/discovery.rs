//! Finding fonts that are installed on the machine.
//!
//! Used by the font fallback in M4: when the embedded subset of a document does
//! not contain a character the user typed, we look for a system font that does
//! and embed a fresh subset of it.
//!
//! The scan is deliberately shallow and cheap. It only records paths and names,
//! it never loads a font program.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFont {
    pub path: PathBuf,
    /// File name without the extension. The real family name needs the font to
    /// be parsed, which the caller does when it actually wants the font.
    pub file_stem: String,
    /// True for `.ttc` and `.otc`, where one file holds several faces.
    pub is_collection: bool,
}

const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];

/// Walk the given directories and list every font file.
pub fn discover_fonts(dirs: &[PathBuf]) -> Vec<SystemFont> {
    let mut found = Vec::new();
    for dir in dirs {
        walk(dir, 0, &mut found);
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found.dedup_by(|a, b| a.path == b.path);
    found
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<SystemFont>) {
    // Font directories nest a few levels at most. A depth limit also protects
    // against a symlink loop.
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            walk(&path, depth + 1, out);
            continue;
        }
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let extension = extension.to_ascii_lowercase();
        if !FONT_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }
        let file_stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(SystemFont {
            is_collection: extension.ends_with('c'),
            file_stem,
            path,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_yields_nothing_and_does_not_fail() {
        let fonts = discover_fonts(&[PathBuf::from("/this/path/does/not/exist")]);
        assert!(fonts.is_empty());
    }

    #[test]
    fn finds_font_files_and_ignores_everything_else() {
        let dir = std::env::temp_dir().join("npdf-font-discovery-test");
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(dir.join("Alpha.ttf"), b"x").unwrap();
        std::fs::write(dir.join("readme.txt"), b"x").unwrap();
        std::fs::write(nested.join("Beta.OTC"), b"x").unwrap();

        let fonts = discover_fonts(std::slice::from_ref(&dir));
        let stems: Vec<&str> = fonts.iter().map(|f| f.file_stem.as_str()).collect();
        assert!(stems.contains(&"Alpha"), "found {stems:?}");
        assert!(stems.contains(&"Beta"), "found {stems:?}");
        assert!(!stems.contains(&"readme"));
        assert!(
            fonts
                .iter()
                .find(|f| f.file_stem == "Beta")
                .unwrap()
                .is_collection
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
