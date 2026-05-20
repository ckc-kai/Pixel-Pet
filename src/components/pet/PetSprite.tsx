// Renders the user's base drawing into a canvas sized to the pet window.
//
// We use a <canvas> instead of a CSS grid of 1024 nodes because:
//   - It paints in a single GPU upload, not per-cell.
//   - We need the drawn pixel data anyway for the click-through hit test;
//     painting + hit-testing share the same source of truth.
//   - `image-rendering: pixelated` preserves crisp pixel edges when scaled.
//
// The canvas is drawn once per drawing change. State-driven visuals
// (filters, breathing, jitter) live on the parent .pet-sprite via CSS —
// the canvas pixels never re-paint for animation.

import { useEffect, useRef } from "react";
import type { Drawing } from "../../lib/types/bindings";

interface PetSpriteProps {
  drawing: Drawing | null | undefined;
  pixelSize: number;
}

/**
 * Parse a palette color string into [r, g, b, a]. Supports `#rgb`, `#rgba`,
 * `#rrggbb`, `#rrggbbaa`. Anything else is treated as fully transparent so
 * a malformed palette degrades to a hole rather than crashing the render.
 */
function parseColor(hex: string): [number, number, number, number] {
  if (typeof hex !== "string" || hex[0] !== "#") return [0, 0, 0, 0];
  const body = hex.slice(1);
  let r = 0;
  let g = 0;
  let b = 0;
  let a = 255;
  if (body.length === 3 || body.length === 4) {
    r = parseInt(body[0] + body[0], 16);
    g = parseInt(body[1] + body[1], 16);
    b = parseInt(body[2] + body[2], 16);
    if (body.length === 4) a = parseInt(body[3] + body[3], 16);
  } else if (body.length === 6 || body.length === 8) {
    r = parseInt(body.slice(0, 2), 16);
    g = parseInt(body.slice(2, 4), 16);
    b = parseInt(body.slice(4, 6), 16);
    if (body.length === 8) a = parseInt(body.slice(6, 8), 16);
  } else {
    return [0, 0, 0, 0];
  }
  if ([r, g, b, a].some((v) => Number.isNaN(v))) return [0, 0, 0, 0];
  return [r, g, b, a];
}

export function PetSprite({ drawing, pixelSize }: PetSpriteProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !drawing) return;
    const { width, height, palette, pixels } = drawing;
    if (width === 0 || height === 0) return;

    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const image = ctx.createImageData(width, height);
    const data = image.data;
    for (let y = 0; y < height; y += 1) {
      const row = pixels[y];
      if (!row) continue;
      for (let x = 0; x < width; x += 1) {
        const idx = row[x] ?? 0;
        const color = palette[idx] ?? palette[0] ?? "#00000000";
        const [r, g, b, a] = parseColor(color);
        const off = (y * width + x) * 4;
        data[off] = r;
        data[off + 1] = g;
        data[off + 2] = b;
        data[off + 3] = a;
      }
    }
    ctx.putImageData(image, 0, 0);
  }, [drawing]);

  // Display dimensions are driven by the window (pixelSize px square).
  // The canvas's internal bitmap stays at the drawing's native size.
  return (
    <canvas
      ref={canvasRef}
      className="pet-canvas"
      style={{
        width: pixelSize,
        height: pixelSize,
        imageRendering: "pixelated",
        display: "block",
      }}
      aria-hidden="true"
    />
  );
}
