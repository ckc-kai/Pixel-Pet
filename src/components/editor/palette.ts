/**
 * Drawing palette — the colors the user paints with, separate from the
 * editor's own UI chrome.
 *
 * Slot 0 is always transparent. Slots 1–15 are a general-purpose tasteful set
 * (DB16-inspired) so a first-time user has enough range to draw a recognizable
 * pet without ever needing a custom-color UI.
 *
 * Hex strings here MUST stay in sync with the `--art-NN` custom properties in
 * `src/styles/tokens.css`. The drawing JSON written to disk stores hex
 * strings, so changing this list later would be a schema-affecting change.
 */

export const TRANSPARENT_INDEX = 0;
export const PALETTE_SIZE = 16;

export const DRAWING_PALETTE: readonly string[] = [
  /* 00 */ "#00000000", // transparent — sentinel; never rendered as a fill
  /* 01 */ "#1a1c2c",
  /* 02 */ "#5d275d",
  /* 03 */ "#b13e53",
  /* 04 */ "#ef7d57",
  /* 05 */ "#ffcd75",
  /* 06 */ "#a7f070",
  /* 07 */ "#38b764",
  /* 08 */ "#257179",
  /* 09 */ "#29366f",
  /* 10 */ "#3b5dc9",
  /* 11 */ "#41a6f6",
  /* 12 */ "#73eff7",
  /* 13 */ "#f4f4f4",
  /* 14 */ "#94b0c2",
  /* 15 */ "#566c86",
] as const;

if (DRAWING_PALETTE.length !== PALETTE_SIZE) {
  throw new Error(
    `DRAWING_PALETTE length (${DRAWING_PALETTE.length}) must equal PALETTE_SIZE (${PALETTE_SIZE})`,
  );
}

/** CSS variable name for a palette slot, mirroring tokens.css. */
export function paletteCssVar(index: number): string {
  return `--art-${index.toString().padStart(2, "0")}`;
}
