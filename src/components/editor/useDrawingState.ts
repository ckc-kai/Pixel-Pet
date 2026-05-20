/**
 * Drawing state hook.
 *
 * Manages the 2D pixel grid plus a single-step undo. Strokes (one
 * mousedown→mouseup gesture) collapse into one undo step — single-pixel
 * undo would be exhausting on a 32×32 canvas.
 *
 * Immutable updates: every paint returns a new outer array; the previous
 * snapshot is kept verbatim for undo.
 */

import { useCallback, useRef, useState } from "react";

import { TRANSPARENT_INDEX } from "./palette";

export type PixelGrid = readonly (readonly number[])[];

interface UseDrawingStateOptions {
  readonly width: number;
  readonly height: number;
}

interface UseDrawingState {
  readonly pixels: PixelGrid;
  readonly canUndo: boolean;
  /** Start a stroke. Captures the current grid as the next undo target. */
  beginStroke(): void;
  /** Paint one pixel inside the active stroke. No-op if coords are out of range. */
  paintPixel(x: number, y: number, paletteIndex: number): void;
  /** End a stroke. Discards the snapshot if the stroke produced no net change. */
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
}: UseDrawingStateOptions): UseDrawingState {
  const [pixels, setPixels] = useState<PixelGrid>(() =>
    makeEmptyGrid(width, height),
  );
  const [undoSnapshot, setUndoSnapshot] = useState<PixelGrid | null>(null);

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
      // touched row (rest of grid shares references — cheap on 32×32).
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
    if (changed) {
      setUndoSnapshot(startGrid);
    }
  }, []);

  const undo = useCallback((): void => {
    setUndoSnapshot((snapshot) => {
      if (!snapshot) return null;
      setPixels(snapshot);
      return null;
    });
  }, []);

  return {
    pixels,
    canUndo: undoSnapshot !== null,
    beginStroke,
    paintPixel,
    endStroke,
    undo,
  };
}
