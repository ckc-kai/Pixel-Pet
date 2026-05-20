/**
 * Editor configuration constants.
 *
 * Kept in one place so canvas dimensions and the schema version are not
 * scattered as magic numbers — CLAUDE.md §3 forbids hardcoded sizes/timings.
 */

/** Canvas width/height in pixels (the user's drawing grid, not the rendered DOM size). */
export const CANVAS_WIDTH = 32;
export const CANVAS_HEIGHT = 32;

/**
 * Drawing schema version. Must match `crate::config::SCHEMA_VERSION` on the
 * Rust side. Persistence rejects drawings whose version is ahead of the
 * server's. The tauri-specta-generated `Drawing` type omits this constant so
 * we own it here on the frontend.
 */
export const DRAWING_SCHEMA_VERSION = 1;

/**
 * Which pet state's drawing this editor produces. The user draws the pet's
 * *appearance*; PR 1.3 will derive the other six states (Sleep, Tired, etc.)
 * from this base. Keep as a typed constant so a future PR that adds direct
 * draw-other-states can override.
 */
export const EDITOR_TARGET_STATE = "working" as const;
