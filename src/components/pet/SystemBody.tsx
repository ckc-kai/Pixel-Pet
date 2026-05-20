// Procedural "body" shape rendered behind the user's drawing.
//
// Why this exists:
//   Most users will draw just a head / face. With only a head, the
//   state animations (especially `stretch`) have nothing visually
//   distinct to do — body squash/stretch reads as random head wobble.
//   The system body gives the pet a stable lower half so motion is
//   legible. It is procedurally drawn from the user's palette so it
//   never clashes with whatever the user picked.
//
// Rendering rules:
//   - Mounted INSIDE `.pet-sprite` below `<PetSprite/>` so it inherits
//     the parent's transform / filter / opacity animations.
//   - Sized off a bbox of the user's opaque pixels (passed in by the
//     parent — keeping bbox computation in one place).
//   - Color is the most-used non-transparent palette entry, with the
//     outline darkened by ~15%. This keeps the body in the user's
//     visual language without us having to guess at intent.
//   - Legs get their own class so the `stretch` state can extend them.
//
// What it does NOT do:
//   - No JS animation, no rAF, no per-frame work. All motion comes from
//     the parent's CSS state class.
//   - Doesn't depend on which state is active — state coupling lives in
//     CSS (`.pet-state-stretch .pet-body-legs { ... }`).

import { useMemo } from "react";
import type { Drawing } from "../../lib/types/bindings";

export interface Bbox {
  /** Inclusive min x in source-pixel coords. */
  minX: number;
  /** Exclusive max x in source-pixel coords. */
  maxX: number;
  /** Inclusive min y in source-pixel coords. */
  minY: number;
  /** Exclusive max y in source-pixel coords. */
  maxY: number;
}

interface SystemBodyProps {
  drawing: Drawing | null | undefined;
  bbox: Bbox | null;
}

/** Parse `#rgb`/`#rrggbb`/`#rgba`/`#rrggbbaa` → `[r,g,b,a]` (0–255). */
function parseHex(hex: string): [number, number, number, number] | null {
  if (typeof hex !== "string" || hex[0] !== "#") return null;
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
    return null;
  }
  if ([r, g, b, a].some((v) => Number.isNaN(v))) return null;
  return [r, g, b, a];
}

function toHex2(n: number): string {
  return n.toString(16).padStart(2, "0");
}

function rgbToHex(r: number, g: number, b: number): string {
  return `#${toHex2(r)}${toHex2(g)}${toHex2(b)}`;
}

/** Darken by ~15% (multiply RGB by 0.85), clamped 0–255. */
function darken(r: number, g: number, b: number): string {
  const f = 0.85;
  return rgbToHex(
    Math.max(0, Math.min(255, Math.round(r * f))),
    Math.max(0, Math.min(255, Math.round(g * f))),
    Math.max(0, Math.min(255, Math.round(b * f))),
  );
}

/**
 * Dominant non-transparent palette entry. Returns `null` if the drawing
 * is empty or every paint is transparent (palette[0] is the transparent
 * slot by convention — see persistence/mod.rs).
 */
function dominantColor(drawing: Drawing): { fill: string; stroke: string } | null {
  const { palette, pixels } = drawing;

  // Build per-palette opacity flags once (mirrors PetRenderer.buildHitMask).
  const isTransparent: boolean[] = palette.map((hex, i) => {
    if (i === 0) return true;
    const parsed = parseHex(hex);
    if (!parsed) return true;
    return parsed[3] === 0;
  });

  const counts = new Array<number>(palette.length).fill(0);
  for (let y = 0; y < pixels.length; y += 1) {
    const row = pixels[y];
    if (!row) continue;
    for (let x = 0; x < row.length; x += 1) {
      const idx = row[x] ?? 0;
      if (idx < 0 || idx >= palette.length) continue;
      if (isTransparent[idx]) continue;
      counts[idx] += 1;
    }
  }

  let bestIdx = -1;
  let bestCount = 0;
  for (let i = 0; i < counts.length; i += 1) {
    if (counts[i] > bestCount) {
      bestCount = counts[i];
      bestIdx = i;
    }
  }
  if (bestIdx < 0) return null;

  const parsed = parseHex(palette[bestIdx]);
  if (!parsed) return null;
  const [r, g, b] = parsed;
  return { fill: rgbToHex(r, g, b), stroke: darken(r, g, b) };
}

// --- Body geometry tuning ------------------------------------------------- //
//
// Numbers picked to keep the body legible at 96×96 windows while still
// looking proportional next to a head that fills most of the bbox.
// Kept as named constants so they aren't anonymous magic.

/** Body width as a multiplier of head bbox width. */
const BODY_WIDTH_FACTOR = 1.2;
/** Body height as a fraction of the canvas space below the head. */
const BODY_HEIGHT_FACTOR = 0.9;
/** Pixel gap between bottom of head and top of body, in source coords. */
const BODY_TOP_GAP = 1;
/** Corner radius as a fraction of the smaller body dimension. */
const BODY_CORNER_RADIUS_FRACTION = 0.35;
/** Leg width / height in source-pixel coords. */
const LEG_WIDTH = 5;
const LEG_HEIGHT = 8;
/** Min body height (source px). Skip rendering below this — no room. */
const MIN_BODY_HEIGHT = 4;

export function SystemBody({ drawing, bbox }: SystemBodyProps) {
  // Memoize the geometry + color so render is essentially free across
  // animation frames (the parent re-renders on state changes but those
  // never change the drawing or bbox).
  const geometry = useMemo(() => {
    if (!drawing || !bbox) return null;
    const colors = dominantColor(drawing);
    if (!colors) return null;

    const canvasW = drawing.width;
    const canvasH = drawing.height;

    const headWidth = bbox.maxX - bbox.minX;
    const headCenterX = (bbox.minX + bbox.maxX) / 2;

    const bodyWidth = Math.min(canvasW, headWidth * BODY_WIDTH_FACTOR);
    const bodyTop = Math.min(canvasH, bbox.maxY + BODY_TOP_GAP);
    const spaceBelow = canvasH - bodyTop;
    const bodyHeight = Math.max(0, spaceBelow * BODY_HEIGHT_FACTOR);

    if (bodyHeight < MIN_BODY_HEIGHT) return null;

    let bodyLeft = headCenterX - bodyWidth / 2;
    // Keep body inside the canvas horizontally so legs don't get clipped.
    if (bodyLeft < 0) bodyLeft = 0;
    if (bodyLeft + bodyWidth > canvasW) bodyLeft = canvasW - bodyWidth;

    const bodyBottom = bodyTop + bodyHeight;
    const cornerRadius =
      Math.min(bodyWidth, bodyHeight) * BODY_CORNER_RADIUS_FRACTION;

    // Legs sit at the bottom of the body, inset from each edge so they
    // read as two distinct stubs rather than the body's lower corners.
    const legInset = Math.max(1, bodyWidth * 0.12);
    const leftLegX = bodyLeft + legInset;
    const rightLegX = bodyLeft + bodyWidth - legInset - LEG_WIDTH;
    const legY = bodyBottom;
    // Clamp legs to canvas
    const legBottomMax = canvasH;
    const legHeight = Math.min(LEG_HEIGHT, Math.max(0, legBottomMax - legY));
    if (legHeight <= 0) {
      return {
        viewBox: `0 0 ${canvasW} ${canvasH}`,
        bodyLeft,
        bodyTop,
        bodyWidth,
        bodyHeight,
        cornerRadius,
        legs: null,
        fill: colors.fill,
        stroke: colors.stroke,
      };
    }

    return {
      viewBox: `0 0 ${canvasW} ${canvasH}`,
      bodyLeft,
      bodyTop,
      bodyWidth,
      bodyHeight,
      cornerRadius,
      legs: {
        leftX: leftLegX,
        rightX: rightLegX,
        y: legY,
        width: LEG_WIDTH,
        height: legHeight,
      },
      fill: colors.fill,
      stroke: colors.stroke,
    };
  }, [drawing, bbox]);

  if (!geometry) return null;

  // Stroke width is in source-pixel units; 1px reads as a crisp outline
  // after the SVG is scaled up by the parent (image-rendering on the SVG
  // wrapper is irrelevant — SVG paths anti-alias regardless).
  const strokeWidth = 1;

  return (
    <svg
      className="pet-system-body"
      viewBox={geometry.viewBox}
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <rect
        className="pet-body-torso"
        x={geometry.bodyLeft}
        y={geometry.bodyTop}
        width={geometry.bodyWidth}
        height={geometry.bodyHeight}
        rx={geometry.cornerRadius}
        ry={geometry.cornerRadius}
        fill={geometry.fill}
        stroke={geometry.stroke}
        strokeWidth={strokeWidth}
      />
      {geometry.legs && (
        <g
          className="pet-body-legs"
          style={{
            // Animate from the top of the legs so they appear to extend
            // downward in stretch state (visually grounded).
            transformOrigin: `50% ${geometry.legs.y}px`,
          }}
        >
          <rect
            x={geometry.legs.leftX}
            y={geometry.legs.y}
            width={geometry.legs.width}
            height={geometry.legs.height}
            fill={geometry.fill}
            stroke={geometry.stroke}
            strokeWidth={strokeWidth}
          />
          <rect
            x={geometry.legs.rightX}
            y={geometry.legs.y}
            width={geometry.legs.width}
            height={geometry.legs.height}
            fill={geometry.fill}
            stroke={geometry.stroke}
            strokeWidth={strokeWidth}
          />
        </g>
      )}
    </svg>
  );
}
