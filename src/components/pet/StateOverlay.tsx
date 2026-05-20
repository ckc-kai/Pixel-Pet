// Per-state overlay decorations.
//
// Two states have extra pixel personality on top of the base sprite:
//   - `tired`  → a small sweat drop pixel cluster
//   - `sleep`  → a stack of zzz glyphs floating up and fading
//
// Positioning: the overlays read the `--pet-bbox-*` CSS custom properties
// set on `.pet-root` by PetRenderer. That lets the sweat drop track the
// top-right of the actually-drawn pixels instead of the window corner.
// When no bbox is available (drawing still loading / empty drawing), the
// CSS defaults fall back to a fixed corner so we still render something.
//
// Overlays are CSS-animated. They never re-render based on time.

import type { PetState } from "../../lib/types/bindings";

interface StateOverlayProps {
  state: PetState;
}

// SVG sweat drop — pixel-art look, 4-color (highlight, body, shadow, outline).
// Kept inline because it never changes; ships zero runtime bytes beyond the
// JSX tree and bypasses any sprite-loading round-trip.
function SweatDrop() {
  return (
    <svg
      className="pet-overlay pet-overlay-sweat"
      viewBox="0 0 8 12"
      shapeRendering="crispEdges"
      aria-hidden="true"
    >
      <rect x="3" y="0" width="2" height="2" fill="#5fb0e8" />
      <rect x="2" y="2" width="4" height="2" fill="#3f9ad3" />
      <rect x="1" y="4" width="6" height="4" fill="#3a8fc8" />
      <rect x="2" y="8" width="4" height="2" fill="#2d7ab0" />
      <rect x="3" y="10" width="2" height="1" fill="#1d6090" />
      <rect x="3" y="1" width="1" height="2" fill="#a5d5f3" />
    </svg>
  );
}

function Zzz() {
  return (
    <div className="pet-overlay pet-overlay-zzz" aria-hidden="true">
      <span>z</span>
      <span>z</span>
      <span>Z</span>
    </div>
  );
}

export function StateOverlay({ state }: StateOverlayProps) {
  switch (state) {
    case "tired":
      return <SweatDrop />;
    case "sleep":
      return <Zzz />;
    default:
      return null;
  }
}
