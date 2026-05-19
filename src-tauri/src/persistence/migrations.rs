//! Schema-version migration framework — currently only registers the v0 → v1
//! identity migration; real migrations are added when schemas break.
//!
//! Owner: agent A4. See `docs/agent-team-plan.md` §4.4 and
//! `docs/architecture.md` §4.5.
//!
//! Design:
//! - Migrations are pure functions on `serde_json::Value`. They take the old
//!   payload, return the upgraded payload, and bump the schema version by one.
//! - The registry is a static slice of `(from, to, fn)` triples. [`migrate`]
//!   walks them sequentially from `current_version` up to `target_version`.
//! - If `current_version > target_version`, we refuse to write — the file was
//!   produced by a newer build than the running code (architecture.md §4.5).
//! - If a hop has no registered migration, we surface the same
//!   [`StorageError::FutureVersion`]: the code does not know how to upgrade.

use serde_json::Value;

use super::StorageError;

/// A single-step migration from `from` → `from + 1`.
type MigrationFn = fn(Value) -> Result<Value, StorageError>;

/// Registry of all known migrations. Each entry covers exactly one version
/// hop; the framework chains them together.
const MIGRATIONS: &[(u32, u32, MigrationFn)] = &[(0, 1, migrate_0_to_1)];

/// Identity migration — placeholder until v1 schema actually diverges from v0.
fn migrate_0_to_1(payload: Value) -> Result<Value, StorageError> {
    Ok(payload)
}

/// Walk the registered migrations from `current_version` up to
/// `target_version`. See module-level docs for the failure modes.
pub fn migrate(
    current_version: u32,
    target_version: u32,
    payload: Value,
) -> Result<Value, StorageError> {
    if current_version > target_version {
        tracing::error!(
            file_version = current_version,
            code_version = target_version,
            "file schema is newer than code; refusing to migrate"
        );
        return Err(StorageError::FutureVersion);
    }
    if current_version == target_version {
        return Ok(payload);
    }

    let mut value = payload;
    let mut version = current_version;
    while version < target_version {
        let next = version + 1;
        let step = MIGRATIONS
            .iter()
            .find(|(from, to, _)| *from == version && *to == next)
            .ok_or_else(|| {
                tracing::error!(
                    from = version,
                    to = next,
                    "no migration registered for required hop"
                );
                StorageError::FutureVersion
            })?;
        value = (step.2)(value)?;
        version = next;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identity_v0_to_v1_preserves_payload() {
        let payload = json!({
            "schema_version": 0,
            "window": { "x": 1, "y": 2 },
            "meals": ["08:00", "12:30", "19:00"]
        });
        let migrated = migrate(0, 1, payload.clone()).expect("migrate");
        assert_eq!(migrated, payload);
    }

    #[test]
    fn same_version_returns_payload_unchanged() {
        let payload = json!({ "ok": true });
        let migrated = migrate(1, 1, payload.clone()).expect("migrate");
        assert_eq!(migrated, payload);
    }

    #[test]
    fn file_newer_than_code_returns_future_version() {
        let payload = json!({ "schema_version": 99 });
        let err = migrate(99, 1, payload).expect_err("should refuse");
        assert!(matches!(err, StorageError::FutureVersion));
    }

    #[test]
    fn missing_hop_returns_future_version() {
        // v1 → v5 is not yet registered; we only know v0 → v1.
        let payload = json!({});
        let err = migrate(1, 5, payload).expect_err("should refuse");
        assert!(matches!(err, StorageError::FutureVersion));
    }
}
