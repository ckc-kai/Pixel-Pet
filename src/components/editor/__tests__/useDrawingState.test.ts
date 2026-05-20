/**
 * Tests for the drawing state hook (CLAUDE.md §4 red line —
 * "pixel canvas serialization" / dimensions / palette handling).
 *
 * Covers:
 * - empty initial grid (dimensions + transparent fill)
 * - immutable paint
 * - stroke lifecycle (begin → paint × N → end) collapses into one undo step
 * - undo restores prior grid; full unwinding returns to the initial state
 * - undo stack cap drops oldest entries
 * - out-of-range coords are no-op
 * - paint outside a stroke is no-op
 */

import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TRANSPARENT_INDEX } from "../palette";
import { useDrawingState, type PixelGrid } from "../useDrawingState";

const WIDTH = 4;
const HEIGHT = 3;

function paint(
  hook: ReturnType<typeof renderHook<ReturnType<typeof useDrawingState>, void>>,
  coords: ReadonlyArray<readonly [number, number, number]>,
): void {
  act(() => {
    hook.result.current.beginStroke();
  });
  for (const [x, y, idx] of coords) {
    act(() => {
      hook.result.current.paintPixel(x, y, idx);
    });
  }
  act(() => {
    hook.result.current.endStroke();
  });
}

function snapshot(grid: PixelGrid): number[][] {
  return grid.map((row) => [...row]);
}

describe("useDrawingState", () => {
  it("starts with a grid sized W×H, filled with the transparent index", () => {
    const { result } = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    expect(result.current.pixels).toHaveLength(HEIGHT);
    for (const row of result.current.pixels) {
      expect(row).toHaveLength(WIDTH);
      for (const cell of row) {
        expect(cell).toBe(TRANSPARENT_INDEX);
      }
    }
    expect(result.current.canUndo).toBe(false);
  });

  it("updates exactly one cell per paint, without mutating prior snapshots", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    const before = snapshot(hook.result.current.pixels);

    paint(hook, [[1, 0, 5]]);

    expect(hook.result.current.pixels[0]?.[1]).toBe(5);
    // Untouched cells stay transparent.
    expect(hook.result.current.pixels[0]?.[0]).toBe(TRANSPARENT_INDEX);
    expect(hook.result.current.pixels[1]?.[1]).toBe(TRANSPARENT_INDEX);
    // The snapshot we captured before painting must not have been mutated.
    expect(before[0]?.[1]).toBe(TRANSPARENT_INDEX);
  });

  it("collapses a multi-pixel stroke into a single undo entry", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    paint(hook, [
      [0, 0, 3],
      [1, 0, 3],
      [2, 0, 3],
    ]);
    expect(hook.result.current.canUndo).toBe(true);

    act(() => {
      hook.result.current.undo();
    });

    // One undo wipes the entire stroke, not just one pixel.
    expect(hook.result.current.canUndo).toBe(false);
    for (const cell of hook.result.current.pixels[0] ?? []) {
      expect(cell).toBe(TRANSPARENT_INDEX);
    }
  });

  it("undoes through the full stack and returns to the initial empty grid", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    const initial = snapshot(hook.result.current.pixels);

    paint(hook, [[0, 0, 1]]);
    paint(hook, [[1, 1, 2]]);
    paint(hook, [[2, 2, 3]]);

    expect(hook.result.current.canUndo).toBe(true);

    act(() => hook.result.current.undo());
    act(() => hook.result.current.undo());
    act(() => hook.result.current.undo());

    expect(hook.result.current.canUndo).toBe(false);
    expect(snapshot(hook.result.current.pixels)).toEqual(initial);
  });

  it("undo while stack is empty is a no-op", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    const before = snapshot(hook.result.current.pixels);
    act(() => hook.result.current.undo());
    expect(snapshot(hook.result.current.pixels)).toEqual(before);
    expect(hook.result.current.canUndo).toBe(false);
  });

  it("drops the oldest snapshot once the stack would exceed the cap", () => {
    const cap = 3;
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT, undoLimit: cap }),
    );

    // Each stroke paints a distinguishable color in (0,0).
    paint(hook, [[0, 0, 1]]); // snapshot: all-empty
    paint(hook, [[0, 0, 2]]); // snapshot: only (0,0)=1
    paint(hook, [[0, 0, 3]]); // snapshot: only (0,0)=2  (stack now: empty, 1, 2)
    // The next stroke pushes us past the cap; oldest (all-empty) is dropped.
    paint(hook, [[0, 0, 4]]); // stack becomes: 1, 2, 3 (length 3)

    expect(hook.result.current.pixels[0]?.[0]).toBe(4);

    // Three undos → grid value should follow 4 → 3 → 2 → 1, then stack empty.
    act(() => hook.result.current.undo());
    expect(hook.result.current.pixels[0]?.[0]).toBe(3);
    act(() => hook.result.current.undo());
    expect(hook.result.current.pixels[0]?.[0]).toBe(2);
    act(() => hook.result.current.undo());
    expect(hook.result.current.pixels[0]?.[0]).toBe(1);

    // Cap was 3 → we should NOT be able to reach the original empty grid.
    expect(hook.result.current.canUndo).toBe(false);
  });

  it("ignores paintPixel with out-of-range coordinates", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    const initial = snapshot(hook.result.current.pixels);

    paint(hook, [
      [-1, 0, 1],
      [WIDTH, 0, 2],
      [0, -1, 3],
      [0, HEIGHT, 4],
      [99, 99, 5],
    ]);

    // No real change → no undo step registered.
    expect(snapshot(hook.result.current.pixels)).toEqual(initial);
    expect(hook.result.current.canUndo).toBe(false);
  });

  it("ignores paintPixel calls made outside an active stroke", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );
    const initial = snapshot(hook.result.current.pixels);

    act(() => {
      hook.result.current.paintPixel(0, 0, 7);
    });
    expect(snapshot(hook.result.current.pixels)).toEqual(initial);
    expect(hook.result.current.canUndo).toBe(false);
  });

  it("does not register an undo step when a stroke produces no net change", () => {
    const hook = renderHook(() =>
      useDrawingState({ width: WIDTH, height: HEIGHT }),
    );

    // Pre-seed a pixel so the next stroke can be a true no-op.
    paint(hook, [[0, 0, 5]]);
    expect(hook.result.current.canUndo).toBe(true);

    // Stroke paints the same color over the same cell → no change.
    paint(hook, [[0, 0, 5]]);

    // canUndo still reflects the one earlier real change, not two.
    act(() => hook.result.current.undo());
    expect(hook.result.current.canUndo).toBe(false);
    expect(hook.result.current.pixels[0]?.[0]).toBe(TRANSPARENT_INDEX);
  });
});
