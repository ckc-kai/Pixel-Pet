//! Per-state JSON drawing loader + atomic writer with palette bounds check.
//!
//! Owner: agent A4. See `docs/agent-team-plan.md` §4.4 and
//! `docs/architecture.md` §4.4.
//!
//! Layout on disk:
//!
//! ```text
//! <dir>/drawings/<state-slug>.json
//! ```
//!
//! Each load enforces structural invariants from architecture.md §4.4:
//! row/column counts match declared `width` / `height`, palette indices stay
//! in range, and dimensions stay within [`DRAWING_MAX_DIMENSION`]. Any
//! violation is mapped to [`StorageError::Corrupt`] — never a panic.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use super::{Drawing, StorageError};
use crate::config::DRAWING_MAX_DIMENSION;
use crate::state::PetState;

/// Subdirectory under the app data dir that holds per-state drawings.
const DRAWINGS_SUBDIR: &str = "drawings";

/// Suffix appended to the previous file before any atomic overwrite.
const BACKUP_SUFFIX: &str = ".bak";

/// File extension used for drawing payloads.
const DRAWING_EXT: &str = "json";

fn drawings_dir(dir: &Path) -> PathBuf {
    dir.join(DRAWINGS_SUBDIR)
}

fn drawing_path(dir: &Path, state: PetState) -> PathBuf {
    drawings_dir(dir).join(format!("{}.{DRAWING_EXT}", state.slug()))
}

fn backup_path_for(dir: &Path, state: PetState) -> PathBuf {
    drawings_dir(dir).join(format!("{}.{DRAWING_EXT}{BACKUP_SUFFIX}", state.slug()))
}

/// Load the drawing for `state` from `dir/drawings/<slug>.json`.
///
/// - File absent → `Ok(None)` (caller falls back to a built-in default).
/// - File present but structurally invalid → [`StorageError::Corrupt`].
/// - File present but unreadable / unparseable → [`StorageError::Io`] /
///   [`StorageError::Encoding`].
pub fn load_drawing(dir: &Path, state: PetState) -> Result<Option<Drawing>, StorageError> {
    let path = drawing_path(dir, state);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            tracing::error!(error = %err, "failed to read drawing file");
            return Err(StorageError::Io);
        }
    };

    let drawing: Drawing = serde_json::from_str(&content).map_err(|err| {
        tracing::error!(error = %err, "failed to parse drawing JSON");
        StorageError::Encoding
    })?;

    validate(&drawing)?;
    Ok(Some(drawing))
}

/// Persist `drawing` to `dir/drawings/<slug>.json` atomically.
///
/// Mirrors `save_settings`: `mkdir`, copy old to `.bak` if present,
/// write a tempfile in the same directory, then atomic rename.
pub fn save_drawing(dir: &Path, drawing: &Drawing) -> Result<(), StorageError> {
    validate(drawing)?;

    let subdir = drawings_dir(dir);
    fs::create_dir_all(&subdir).map_err(|err| {
        tracing::error!(error = %err, "failed to create drawings directory");
        StorageError::Io
    })?;

    let target = drawing_path(dir, drawing.state);
    if target.exists() {
        let backup = backup_path_for(dir, drawing.state);
        fs::copy(&target, &backup).map_err(|err| {
            tracing::error!(error = %err, "failed to write drawing backup");
            StorageError::Io
        })?;
    }

    let serialized = serde_json::to_string_pretty(drawing).map_err(|err| {
        tracing::error!(error = %err, "failed to serialize drawing");
        StorageError::Encoding
    })?;

    let mut tmp = NamedTempFile::new_in(&subdir).map_err(|err| {
        tracing::error!(error = %err, "failed to create temporary drawing file");
        StorageError::Io
    })?;

    tmp.write_all(serialized.as_bytes()).map_err(|err| {
        tracing::error!(error = %err, "failed to write temporary drawing file");
        StorageError::Io
    })?;

    tmp.persist(&target).map_err(|err| {
        tracing::error!(error = %err.error, "failed to persist drawing file");
        StorageError::Io
    })?;

    Ok(())
}

/// Structural invariants documented in architecture.md §4.4.
fn validate(drawing: &Drawing) -> Result<(), StorageError> {
    if drawing.width > DRAWING_MAX_DIMENSION || drawing.height > DRAWING_MAX_DIMENSION {
        tracing::error!(
            width = drawing.width,
            height = drawing.height,
            max = DRAWING_MAX_DIMENSION,
            "drawing exceeds DRAWING_MAX_DIMENSION"
        );
        return Err(StorageError::Corrupt);
    }

    if drawing.pixels.len() as u64 != drawing.height as u64 {
        tracing::error!(
            row_count = drawing.pixels.len(),
            declared_height = drawing.height,
            "drawing row count does not match declared height"
        );
        return Err(StorageError::Corrupt);
    }

    let palette_len = drawing.palette.len() as u32;
    for (row_idx, row) in drawing.pixels.iter().enumerate() {
        if row.len() as u64 != drawing.width as u64 {
            tracing::error!(
                row_idx,
                row_len = row.len(),
                declared_width = drawing.width,
                "drawing column count does not match declared width"
            );
            return Err(StorageError::Corrupt);
        }
        for (col_idx, &index) in row.iter().enumerate() {
            if index >= palette_len {
                tracing::error!(
                    row_idx,
                    col_idx,
                    palette_index = index,
                    palette_len,
                    "drawing pixel references out-of-range palette index"
                );
                return Err(StorageError::Corrupt);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::config::SCHEMA_VERSION;

    fn make_drawing(state: PetState, width: u32, height: u32) -> Drawing {
        let palette = vec![
            "#00000000".to_string(),
            "#1a1a1a".to_string(),
            "#ffffff".to_string(),
        ];
        let pixels = (0..height)
            .map(|y| (0..width).map(|x| (x + y) % 3).collect())
            .collect();
        Drawing {
            schema_version: SCHEMA_VERSION,
            state,
            width,
            height,
            palette,
            pixels,
        }
    }

    #[test]
    fn load_missing_drawing_returns_none() {
        let dir = TempDir::new().expect("tempdir");
        let loaded = load_drawing(dir.path(), PetState::Working).expect("load");
        assert!(loaded.is_none());
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = TempDir::new().expect("tempdir");
        let drawing = make_drawing(PetState::Working, 4, 4);
        save_drawing(dir.path(), &drawing).expect("save");

        let loaded = load_drawing(dir.path(), PetState::Working)
            .expect("load")
            .expect("present");
        assert_eq!(loaded, drawing);
    }

    #[test]
    fn save_uses_per_state_slug_filenames() {
        let dir = TempDir::new().expect("tempdir");
        let working = make_drawing(PetState::Working, 2, 2);
        let sleep = make_drawing(PetState::Sleep, 2, 2);
        save_drawing(dir.path(), &working).expect("save working");
        save_drawing(dir.path(), &sleep).expect("save sleep");

        let working_path = dir.path().join("drawings/working.json");
        let sleep_path = dir.path().join("drawings/sleep.json");
        assert!(working_path.exists());
        assert!(sleep_path.exists());
    }

    #[test]
    fn load_palette_oob_returns_corrupt() {
        let dir = TempDir::new().expect("tempdir");
        let mut drawing = make_drawing(PetState::Tired, 2, 2);
        // Palette has 3 entries → index 3 is OOB.
        drawing.pixels[0][0] = drawing.palette.len() as u32;

        // Bypass validate() by writing the file directly.
        fs::create_dir_all(drawings_dir(dir.path())).expect("mkdir");
        fs::write(
            drawing_path(dir.path(), PetState::Tired),
            serde_json::to_string(&drawing).expect("serialize"),
        )
        .expect("write");

        let err = load_drawing(dir.path(), PetState::Tired).expect_err("should reject");
        assert!(matches!(err, StorageError::Corrupt));
    }

    #[test]
    fn load_dimension_overflow_returns_corrupt() {
        let dir = TempDir::new().expect("tempdir");
        let over = DRAWING_MAX_DIMENSION + 1;
        let drawing = Drawing {
            schema_version: SCHEMA_VERSION,
            state: PetState::Working,
            width: over,
            height: 1,
            palette: vec!["#000000".to_string()],
            pixels: vec![vec![0; over as usize]],
        };

        fs::create_dir_all(drawings_dir(dir.path())).expect("mkdir");
        fs::write(
            drawing_path(dir.path(), PetState::Working),
            serde_json::to_string(&drawing).expect("serialize"),
        )
        .expect("write");

        let err = load_drawing(dir.path(), PetState::Working).expect_err("should reject");
        assert!(matches!(err, StorageError::Corrupt));
    }

    #[test]
    fn load_row_count_mismatch_returns_corrupt() {
        let dir = TempDir::new().expect("tempdir");
        let drawing = Drawing {
            schema_version: SCHEMA_VERSION,
            state: PetState::Working,
            width: 2,
            height: 3, // claims 3 rows
            palette: vec!["#000".to_string(), "#fff".to_string()],
            pixels: vec![vec![0, 1], vec![1, 0]], // only 2 rows
        };

        fs::create_dir_all(drawings_dir(dir.path())).expect("mkdir");
        fs::write(
            drawing_path(dir.path(), PetState::Working),
            serde_json::to_string(&drawing).expect("serialize"),
        )
        .expect("write");

        let err = load_drawing(dir.path(), PetState::Working).expect_err("should reject");
        assert!(matches!(err, StorageError::Corrupt));
    }

    #[test]
    fn load_column_width_mismatch_returns_corrupt() {
        let dir = TempDir::new().expect("tempdir");
        let drawing = Drawing {
            schema_version: SCHEMA_VERSION,
            state: PetState::Working,
            width: 3, // claims 3 columns
            height: 2,
            palette: vec!["#000".to_string(), "#fff".to_string()],
            pixels: vec![vec![0, 1], vec![1, 0]], // rows are length 2
        };

        fs::create_dir_all(drawings_dir(dir.path())).expect("mkdir");
        fs::write(
            drawing_path(dir.path(), PetState::Working),
            serde_json::to_string(&drawing).expect("serialize"),
        )
        .expect("write");

        let err = load_drawing(dir.path(), PetState::Working).expect_err("should reject");
        assert!(matches!(err, StorageError::Corrupt));
    }

    #[test]
    fn save_rejects_invalid_drawing() {
        let dir = TempDir::new().expect("tempdir");
        let mut drawing = make_drawing(PetState::Working, 2, 2);
        drawing.pixels[0][0] = drawing.palette.len() as u32;
        let err = save_drawing(dir.path(), &drawing).expect_err("should reject");
        assert!(matches!(err, StorageError::Corrupt));

        // Nothing should have been written.
        let target = drawing_path(dir.path(), PetState::Working);
        assert!(!target.exists());
    }

    #[test]
    fn save_writes_bak_of_preexisting_drawing() {
        let dir = TempDir::new().expect("tempdir");
        let first = make_drawing(PetState::Stretch, 2, 2);
        save_drawing(dir.path(), &first).expect("first save");

        let mut second = make_drawing(PetState::Stretch, 2, 2);
        second.palette.push("#e63946".to_string());
        second.pixels[0][0] = 3;
        save_drawing(dir.path(), &second).expect("second save");

        let bak = backup_path_for(dir.path(), PetState::Stretch);
        assert!(bak.exists(), "backup should exist");
        let bak_content = fs::read_to_string(&bak).expect("read bak");
        let bak_parsed: Drawing = serde_json::from_str(&bak_content).expect("parse bak");
        assert_eq!(bak_parsed, first);

        let loaded = load_drawing(dir.path(), PetState::Stretch)
            .expect("load")
            .expect("present");
        assert_eq!(loaded, second);
    }
}
