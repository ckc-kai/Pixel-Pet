//! Unit tests for the IPC layer. Logic that depends on `tauri::State` is
//! exercised via the extracted helpers (`merge_settings_patch`,
//! `settings::apply_patch`, `editor::validate_save_request`) so the tests
//! stay pure and fast.

use serde_json::json;

use super::{merge_settings_patch, IpcError};
use crate::config::Settings;
use crate::ipc::editor::validate_save_request;
use crate::ipc::settings::apply_patch;
use crate::persistence::{Drawing, StorageError};
use crate::state::PetState;

// ---------------------------------------------------------------------------
// merge_settings_patch
// ---------------------------------------------------------------------------

#[test]
fn merge_replaces_single_top_level_field_and_leaves_others_untouched() {
    let base = json!({
        "a": 1,
        "b": 2,
        "c": { "nested": true }
    });
    let patch = json!({ "a": 99 });

    let merged = merge_settings_patch(base, patch);

    assert_eq!(merged["a"], json!(99));
    assert_eq!(merged["b"], json!(2));
    assert_eq!(merged["c"], json!({ "nested": true }));
}

#[test]
fn merge_empty_patch_returns_base_unchanged() {
    let base = json!({ "a": 1, "b": { "c": 2 } });
    let merged = merge_settings_patch(base.clone(), json!({}));
    assert_eq!(merged, base);
}

#[test]
fn merge_nested_field_preserves_sibling_keys() {
    // Reproduces a bug we want to guard against: a naive `Object::insert`
    // would replace the whole `activity` table and wipe `idle_threshold_seconds`.
    let base = json!({
        "activity": {
            "idle_threshold_seconds": 60,
            "poll_interval_seconds": 60,
            "spaced_out_idle_minutes": 15,
        }
    });
    let patch = json!({
        "activity": { "poll_interval_seconds": 30 }
    });

    let merged = merge_settings_patch(base, patch);

    assert_eq!(merged["activity"]["poll_interval_seconds"], json!(30));
    assert_eq!(merged["activity"]["idle_threshold_seconds"], json!(60));
    assert_eq!(merged["activity"]["spaced_out_idle_minutes"], json!(15));
}

#[test]
fn merge_handles_deeply_nested_keys_without_dropping_siblings() {
    let base = json!({
        "work": {
            "stretch_at_minutes": 60,
            "tired_at_minutes": 75,
            "sleep_at_minutes": 90,
            "stretch_overlay_seconds": 30,
            "eating_overlay_seconds": 60,
        }
    });
    let patch = json!({
        "work": { "stretch_at_minutes": 30 }
    });

    let merged = merge_settings_patch(base, patch);

    assert_eq!(merged["work"]["stretch_at_minutes"], json!(30));
    assert_eq!(merged["work"]["tired_at_minutes"], json!(75));
    assert_eq!(merged["work"]["sleep_at_minutes"], json!(90));
    assert_eq!(merged["work"]["stretch_overlay_seconds"], json!(30));
    assert_eq!(merged["work"]["eating_overlay_seconds"], json!(60));
}

// ---------------------------------------------------------------------------
// settings::apply_patch
// ---------------------------------------------------------------------------

#[test]
fn apply_patch_updates_nested_activity_field() {
    let current = Settings::default();
    let patch = json!({
        "activity": { "poll_interval_seconds": 30 }
    });

    let merged = apply_patch(&current, patch).expect("merge");

    assert_eq!(merged.activity.poll_interval_seconds, 30);
    // Sibling fields preserved.
    assert_eq!(
        merged.activity.idle_threshold_seconds,
        current.activity.idle_threshold_seconds
    );
}

#[test]
fn apply_patch_empty_returns_current_settings() {
    let current = Settings::default();
    let merged = apply_patch(&current, json!({})).expect("merge");
    assert_eq!(merged, current);
}

#[test]
fn apply_patch_rejects_value_below_validation_floor() {
    let current = Settings::default();
    // 0 is below POLL_INTERVAL_FLOOR_SECONDS (5).
    let patch = json!({
        "activity": { "poll_interval_seconds": 0 }
    });

    let err = apply_patch(&current, patch).expect_err("must reject");
    match err {
        IpcError::BadRequest(msg) => {
            assert!(
                msg.to_lowercase().contains("poll_interval_seconds"),
                "expected message about poll_interval_seconds, got: {msg}"
            );
        }
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// editor::validate_save_request
// ---------------------------------------------------------------------------

fn make_drawing(state: PetState, width: u32, height: u32) -> Drawing {
    let palette = vec!["#00000000".to_string(), "#ffffff".to_string()];
    let pixels = (0..height)
        .map(|_| (0..width).map(|_| 0_u32).collect())
        .collect();
    Drawing {
        schema_version: crate::config::SCHEMA_VERSION,
        state,
        width,
        height,
        palette,
        pixels,
    }
}

#[test]
fn editor_save_drawing_rejects_oversized_canvas() {
    let drawing = make_drawing(PetState::Working, 257, 1);
    let err = validate_save_request(PetState::Working, &drawing).expect_err("must reject");
    assert!(matches!(err, IpcError::BadRequest(_)));
}

#[test]
fn editor_save_drawing_accepts_max_dimensions() {
    let drawing = make_drawing(PetState::Working, 256, 256);
    validate_save_request(PetState::Working, &drawing).expect("256x256 is the cap, must accept");
}

#[test]
fn editor_save_drawing_accepts_one_by_one() {
    let drawing = make_drawing(PetState::Working, 1, 1);
    validate_save_request(PetState::Working, &drawing).expect("1x1 must accept");
}

#[test]
fn editor_save_drawing_rejects_state_mismatch() {
    let drawing = make_drawing(PetState::Sleep, 8, 8);
    let err = validate_save_request(PetState::Working, &drawing).expect_err("must reject");
    assert!(matches!(err, IpcError::BadRequest(_)));
}

// ---------------------------------------------------------------------------
// IpcError serialization — privacy contract from CLAUDE.md §9
// ---------------------------------------------------------------------------

#[test]
fn ipc_error_storage_serializes_without_filesystem_detail() {
    // Round-trip an IpcError::Storage through serde and assert the wire form
    // does not leak path / io detail. The persistence layer logs full detail
    // server-side; the wire-format must be opaque.
    let from_storage: IpcError = StorageError::Io.into();
    let serialized = serde_json::to_string(&from_storage).expect("serialize");

    assert!(!serialized.contains("/"), "must not leak filesystem paths");
    assert!(
        !serialized.to_lowercase().contains("no such file"),
        "must not leak io::Error text"
    );
    assert!(
        !serialized.to_lowercase().contains("permission denied"),
        "must not leak io::Error text"
    );
    // Schema is `{"kind":"Storage"}` — Storage has no `message` payload.
    assert_eq!(serialized, r#"{"kind":"Storage"}"#);
}

#[test]
fn ipc_error_not_found_serializes_with_kind_and_message() {
    let err = IpcError::NotFound("foo".into());
    let serialized = serde_json::to_string(&err).expect("serialize");
    assert_eq!(serialized, r#"{"kind":"NotFound","message":"foo"}"#);
}

#[test]
fn ipc_error_bad_request_serializes_with_kind_and_message() {
    let err = IpcError::BadRequest("bar".into());
    let serialized = serde_json::to_string(&err).expect("serialize");
    assert_eq!(serialized, r#"{"kind":"BadRequest","message":"bar"}"#);
}

#[test]
fn ipc_error_internal_has_no_message_payload() {
    let serialized = serde_json::to_string(&IpcError::Internal).expect("serialize");
    assert_eq!(serialized, r#"{"kind":"Internal"}"#);
}
