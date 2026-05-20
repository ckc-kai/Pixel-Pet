/**
 * Pixel canvas — 32×32 grid rendered at 10× scale.
 *
 * Pointer-event driven (works with mouse, trackpad, touch). The parent
 * supplies the working pixel grid plus paint callbacks; this component owns
 * coordinate math and the DOM cells.
 */

import { useCallback, useMemo, useRef } from "react";

import {
  DRAWING_PALETTE,
  TRANSPARENT_INDEX,
  paletteCssVar,
} from "./palette";
import type { PixelGrid } from "./useDrawingState";

interface PixelCanvasProps {
  readonly pixels: PixelGrid;
  readonly width: number;
  readonly height: number;
  readonly onStrokeBegin: () => void;
  readonly onStrokePaint: (x: number, y: number) => void;
  readonly onStrokeEnd: () => void;
}

export function PixelCanvas({
  pixels,
  width,
  height,
  onStrokeBegin,
  onStrokePaint,
  onStrokeEnd,
}: PixelCanvasProps) {
  const gridRef = useRef<HTMLDivElement | null>(null);
  const paintingRef = useRef(false);

  const cellFromEvent = useCallback(
    (clientX: number, clientY: number): { x: number; y: number } | null => {
      const grid = gridRef.current;
      if (!grid) return null;
      const rect = grid.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) return null;
      const relX = (clientX - rect.left) / rect.width;
      const relY = (clientY - rect.top) / rect.height;
      if (relX < 0 || relX >= 1 || relY < 0 || relY >= 1) return null;
      const x = Math.floor(relX * width);
      const y = Math.floor(relY * height);
      return { x, y };
    },
    [width, height],
  );

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>): void => {
      // Only react to primary button presses (left mouse / single finger).
      if (event.button !== 0 && event.pointerType === "mouse") return;
      const cell = cellFromEvent(event.clientX, event.clientY);
      if (!cell) return;
      paintingRef.current = true;
      event.currentTarget.setPointerCapture(event.pointerId);
      onStrokeBegin();
      onStrokePaint(cell.x, cell.y);
    },
    [cellFromEvent, onStrokeBegin, onStrokePaint],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>): void => {
      if (!paintingRef.current) return;
      const cell = cellFromEvent(event.clientX, event.clientY);
      if (!cell) return;
      onStrokePaint(cell.x, cell.y);
    },
    [cellFromEvent, onStrokePaint],
  );

  const finishStroke = useCallback((): void => {
    if (!paintingRef.current) return;
    paintingRef.current = false;
    onStrokeEnd();
  }, [onStrokeEnd]);

  const gridStyle = useMemo<React.CSSProperties>(
    () =>
      ({
        // Make sure the CSS grid columns/rows respect our actual canvas size.
        ["--canvas-cells" as string]: String(width),
      }) as React.CSSProperties,
    [width],
  );

  return (
    <div className="canvas-frame">
      <div
        ref={gridRef}
        className="canvas-grid"
        style={gridStyle}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishStroke}
        onPointerCancel={finishStroke}
        onPointerLeave={finishStroke}
        role="img"
        aria-label="Pixel art canvas"
      >
        {pixels.flatMap((row, y) =>
          row.map((paletteIndex, x) => {
            const isTransparent = paletteIndex === TRANSPARENT_INDEX;
            const style: React.CSSProperties = isTransparent
              ? { background: "transparent" }
              : { background: `var(${paletteCssVar(paletteIndex)})` };
            return (
              <div
                key={`${y}-${x}`}
                className="canvas-cell"
                style={style}
                aria-hidden="true"
              />
            );
          }),
        )}
      </div>
    </div>
  );
}

/**
 * Re-export the palette length so callers can sanity-check pixel values
 * before serializing.
 */
export const ACTIVE_PALETTE_LENGTH = DRAWING_PALETTE.length;
