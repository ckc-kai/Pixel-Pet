/**
 * Drawing state hook.
 *
 * Manages the 2D pixel grid plus a bounded undo stack. Strokes (one
 * mousedown→mouseup gesture) collapse into one undo step — single-pixel
 * undo would be exhausting on a 64×64 canvas.
 *
 * Immutable updates: every paint returns a new outer array; the previous
 * snapshot is kept verbatim for undo. The stack is capped at
 * `UNDO_STACK_LIMIT`; when it would grow past that, the oldest snapshot is
 * dropped (shift) so memory stays bounded.
 *
 * No redo by design — explicit product decision (PR B).
 */

import { useCallback, useRef, useState } from "react";

import { UNDO_STACK_LIMIT } from "./constants";
import { TRANSPARENT_INDEX } from "./palette";

export type PixelGrid = readonly (readonly number[])[];

interface UseDrawingStateOptions {
  readonly width: number;
  readonly height: number;
  /** Override the cap (tests only). Defaults to `UNDO_STACK_LIMIT`. */
  readonly undoLimit?: number;
}

interface UseDrawingState {
  readonly pixels: PixelGrid;
  readonly canUndo: boolean;
  /** Start a stroke. Captures the current grid as the next undo target. */
  beginStroke(): void;
  /** Paint one pixel inside the active stroke. No-op if coords are out of range. */
  paintPixel(x: number, y: number, paletteIndex: number): void;
  /** End a stroke. Pushes the snapshot if the stroke produced any net change. */
  endStroke(): void;
  /** Restore the previous grid. No-op if `canUndo` is false. */
  undo(): void;
}

function makeEmptyGrid(width: number, height: number): PixelGrid {
  const rows: number[][] = [];
  for (let y = 0; y < height; y += 1) {
    rows.push(new Array<number>(width).fill(TRANSPARENT_INDEX));
  }
  return rows;
}

export function useDrawingState({
  width,
  height,
  undoLimit = UNDO_STACK_LIMIT,
}: UseDrawingStateOptions): UseDrawingState {
  const [pixels, setPixels] = useState<PixelGrid>(() =>
    makeEmptyGrid(width, height),
  );
  const [undoStack, setUndoStack] = useState<readonly PixelGrid[]>([]);

  // Within a stroke we keep a mutable working copy so per-pixel paint stays
  // O(1) instead of cloning the whole grid on every mousemove. We still
  // publish immutable snapshots to React state.
  const strokeWorkingRef = useRef<number[][] | null>(null);
  const strokeStartRef = useRef<PixelGrid | null>(null);

  const beginStroke = useCallback((): void => {
    // Clone the current grid into a mutable buffer for the stroke.
    const working = pixels.map((row) => [...row]);
    strokeWorkingRef.current = working;
    strokeStartRef.current = pixels;
  }, [pixels]);

  const paintPixel = useCallback(
    (x: number, y: number, paletteIndex: number): void => {
      const working = strokeWorkingRef.current;
      if (!working) return;
      if (y < 0 || y >= working.length) return;
      const row = working[y];
      if (!row || x < 0 || x >= row.length) return;
      if (row[x] === paletteIndex) return;
      row[x] = paletteIndex;
      // Publish an immutable view so consumers re-render. We copy only the
      // touched row (rest of grid shares references — cheap even on 64×64).
      setPixels((prev) =>
        prev.map((prevRow, idx) => (idx === y ? [...row] : prevRow)),
      );
    },
    [],
  );

  const endStroke = useCallback((): void => {
    const startGrid = strokeStartRef.current;
    const working = strokeWorkingRef.current;
    strokeWorkingRef.current = null;
    strokeStartRef.current = null;
    if (!startGrid || !working) return;

    // Only register an undo step if the stroke actually changed something.
    const changed = working.some((row, y) =>
      row.some((cell, x) => cell !== startGrid[y]?.[x]),
    );
    if (!changed) return;

    setUndoStack((prev) => {
      const next = [...prev, startGrid];
      // Drop oldest entries if we'd exceed the cap.
      if (next.length > undoLimit) {
        return next.slice(next.length - undoLimit);
      }
      return next;
    });
  }, [undoLimit]);

  const undo = useCallback((): void => {
    setUndoStack((prev) => {
      if (prev.length === 0) return prev;
      const snapshot = prev[prev.length - 1];
      if (snapshot) {
        setPixels(snapshot);
      }
      return prev.slice(0, -1);
    });
  }, []);

  return {
    pixels,
    canUndo: undoStack.length > 0,
    beginStroke,
    paintPixel,
    endStroke,
    undo,
  };
}
