// Top-level renderer for the pet window.
//
// Responsibilities:
//   1. Subscribe to PetState (via usePetState).
//   2. Load the user's "working" drawing once (via useDrawing("working")).
//      All 7 states render from the same base — non-working states are
//      visual derivations (filter + overlay), not separate drawings.
//   3. Compute a hit-mask + bbox of the user's opaque pixels:
//        - Hit-mask drives click-through (cursor over transparent → window
//          ignores pointer events so clicks pass through to apps below).
//        - Bbox positions overlays (sweat / zzz) and sizes the procedural
//          system body so they track the actual drawn area instead of the
//          window corner.
//   4. Render the layered sprite stack: system body (back), user drawing
//      (front), state overlay (positioned via CSS custom properties).
//
// CSS handles every visual animation (see styles/pet-states.css). No
// requestAnimationFrame is used for paint. The single rAF in this file
// throttles the hit-test handler, which is a pointer/coalescing concern,
// not a render concern.

import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Drawing, PetState } from "../../lib/types/bindings";
import { useDrawing } from "../../hooks/useDrawing";
import { usePetState } from "../../hooks/usePetState";
import { PetCloseButton } from "./PetCloseButton";
import { PetDevPanel } from "./PetDevPanel";
import { PetSprite } from "./PetSprite";
import { StateOverlay } from "./StateOverlay";
import { SystemBody, type Bbox } from "./SystemBody";
import "../../styles/pet-states.css";

// State the renderer defaults to before the first IPC message arrives.
// The backend seeds the channel on subscribe so this is only ever shown
// for a few frames; keeping it as `working` (the canonical base) avoids a
// jarring filter flash on mount.
const DEFAULT_STATE: PetState = "working";

/**
 * Build a fast Uint8Array hit-mask sized to the drawing.
 *
 * `mask[y * width + x] = 1` ⇔ that source pixel is non-transparent and
 * therefore should capture cursor events. A pixel counts as transparent
 * when its palette index is 0 (the convention noted in
 * persistence/mod.rs) OR when its palette entry's alpha component is
 * `0x00`.
 */
function buildHitMask(drawing: Drawing): Uint8Array {
  const { width, height, palette, pixels } = drawing;
  const mask = new Uint8Array(width * height);

  // Per-palette transparency lookup — computed once, then indexed per pixel.
  const transparent: boolean[] = palette.map((hex, i) => {
    if (i === 0) return true; // palette[0] is the transparent slot
    if (typeof hex !== "string" || hex[0] !== "#") return true;
    const body = hex.slice(1);
    if (body.length === 8) {
      const a = parseInt(body.slice(6, 8), 16);
      return Number.isNaN(a) || a === 0;
    }
    if (body.length === 4) {
      const a = parseInt(body[3] + body[3], 16);
      return Number.isNaN(a) || a === 0;
    }
    return false; // #rgb / #rrggbb — opaque
  });

  for (let y = 0; y < height; y += 1) {
    const row = pixels[y];
    if (!row) continue;
    for (let x = 0; x < width; x += 1) {
      const idx = row[x] ?? 0;
      mask[y * width + x] = transparent[idx] ? 0 : 1;
    }
  }
  return mask;
}

/**
 * Bounding box of opaque pixels (`mask[i] === 1`). Returns `null` when
 * the drawing is entirely transparent — callers should fall back to
 * fixed positioning in that case.
 *
 * `maxX` / `maxY` are exclusive, matching slice/range conventions —
 * `bbox.maxX - bbox.minX` is the width in source pixels.
 */
function computeBbox(
  mask: Uint8Array,
  width: number,
  height: number,
): Bbox | null {
  let minX = width;
  let minY = height;
  let maxX = -1;
  let maxY = -1;
  for (let y = 0; y < height; y += 1) {
    const rowStart = y * width;
    for (let x = 0; x < width; x += 1) {
      if (mask[rowStart + x]) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }
  if (maxX < 0) return null;
  return { minX, maxX: maxX + 1, minY, maxY: maxY + 1 };
}

export function PetRenderer() {
  const state = usePetState() ?? DEFAULT_STATE;
  const drawing = useDrawing("working");

  // ---- Window dimensions (dynamic) -------------------------------------- //
  //
  // The Tauri window size is no longer hardcoded (was 64×64, now configurable
  // via settings — default 96×96). Track innerWidth/innerHeight so the
  // hit-test math stays correct if the user resizes via settings later.

  const [winSize, setWinSize] = useState<{ w: number; h: number }>(() => ({
    w: window.innerWidth || 1,
    h: window.innerHeight || 1,
  }));

  useEffect(() => {
    const onResize = () => {
      setWinSize({ w: window.innerWidth || 1, h: window.innerHeight || 1 });
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  // ---- Hit-mask + bbox -------------------------------------------------- //

  const hitMask = useMemo(() => {
    if (!drawing) return null;
    const mask = buildHitMask(drawing);
    const bbox = computeBbox(mask, drawing.width, drawing.height);
    return { mask, width: drawing.width, height: drawing.height, bbox };
  }, [drawing]);

  // CSS custom properties drive overlay positioning so the sprite owns
  // the bbox knowledge and CSS just consumes percentages. When no bbox
  // is available (drawing loading / empty), the overlay falls back to a
  // fixed corner via its CSS default values.
  const bboxStyle = useMemo<React.CSSProperties | undefined>(() => {
    if (!hitMask?.bbox) return undefined;
    const { bbox, width, height } = hitMask;
    return {
      "--pet-bbox-top": `${(bbox.minY / height) * 100}%`,
      "--pet-bbox-right": `${((width - bbox.maxX) / width) * 100}%`,
      "--pet-bbox-bottom": `${((height - bbox.maxY) / height) * 100}%`,
      "--pet-bbox-left": `${(bbox.minX / width) * 100}%`,
      "--pet-bbox-center-x": `${
        ((bbox.minX + bbox.maxX) / 2 / width) * 100
      }%`,
    } as React.CSSProperties;
  }, [hitMask]);

  // ---- Click-through --------------------------------------------------- //
  //
  // We listen for `mousemove` on the document and translate client coords
  // → drawing coords (divide by the per-axis scale). A rAF coalesces bursts
  // of move events so we only call setIgnoreCursorEvents at the display
  // refresh rate. This is hit-testing, NOT a render loop — the pet's
  // visual state never depends on rAF.

  const ignoreRef = useRef<boolean | null>(null);
  const rafRef = useRef<number | null>(null);
  const lastEventRef = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!hitMask) return;
    const { mask, width, height } = hitMask;
    const win = getCurrentWindow();

    const apply = (next: boolean) => {
      if (ignoreRef.current === next) return;
      ignoreRef.current = next;
      win.setIgnoreCursorEvents(next).catch((err) => {
        // eslint-disable-next-line no-console
        console.warn("setIgnoreCursorEvents failed", err);
      });
    };

    const evaluate = () => {
      rafRef.current = null;
      const ev = lastEventRef.current;
      if (!ev) return;
      // Map document coords → source-pixel coords via per-axis scale.
      // Reads from `winSize` (state) rather than the DOM to avoid forcing
      // a reflow on every mousemove.
      const scaleX = width / winSize.w;
      const scaleY = height / winSize.h;
      const sx = Math.floor(ev.x * scaleX);
      const sy = Math.floor(ev.y * scaleY);
      if (sx < 0 || sy < 0 || sx >= width || sy >= height) {
        apply(true);
        return;
      }
      const opaque = mask[sy * width + sx] === 1;
      apply(!opaque);
    };

    const onMove = (e: MouseEvent) => {
      lastEventRef.current = { x: e.clientX, y: e.clientY };
      if (rafRef.current !== null) return;
      rafRef.current = requestAnimationFrame(evaluate);
    };

    // When the cursor leaves the window entirely, fall back to "capture"
    // so the next enter triggers a re-evaluation (otherwise a stuck
    // ignore=true would never recover until the next mousemove).
    const onLeave = () => {
      lastEventRef.current = null;
      apply(false);
    };

    document.addEventListener("mousemove", onMove, { passive: true });
    document.addEventListener("mouseleave", onLeave, { passive: true });

    return () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseleave", onLeave);
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      // Re-enable cursor events on teardown so a future remount isn't stuck
      // with a window that ignores everything.
      win.setIgnoreCursorEvents(false).catch(() => {});
      ignoreRef.current = null;
    };
  }, [hitMask, winSize.w, winSize.h]);

  // ---- Render ---------------------------------------------------------- //

  // The state class drives both the animation and (where applicable) the
  // filter. CSS owns the visual — the React tree never re-renders for
  // animation frames.
  const stateClass = `pet-state-${state}`;
  const bbox = hitMask?.bbox ?? null;

  return (
    <>
      <div className={`pet-root ${stateClass}`} style={bboxStyle}>
        <div className="pet-sprite">
          {/* System body sits BEHIND the user's drawing so the user's
              pixels always read as the pet's face. */}
          <SystemBody drawing={drawing} bbox={bbox} />
          <PetSprite drawing={drawing} />
        </div>
        <StateOverlay state={state} />
      </div>
      <PetCloseButton />
      <PetDevPanel current={state} />
    </>
  );
}
