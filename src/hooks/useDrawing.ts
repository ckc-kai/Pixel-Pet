// Load the user's drawing for a single pet state.
//
// The "working" drawing is the base sprite — every other state is a
// visual derivation (CSS filter + overlay). We load once on mount; the
// drawing is immutable from the renderer's perspective (the editor owns
// writes via a separate command path).
//
// Returns:
//   - `undefined` — still loading
//   - `null`      — no saved drawing (first-run fallback path) or load failed
//   - Drawing     — loaded successfully

import { useEffect, useState } from "react";
import { commands, type Drawing, type PetState } from "../lib/types/bindings";

export function useDrawing(state: PetState): Drawing | null | undefined {
  const [drawing, setDrawing] = useState<Drawing | null | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    commands.editorLoadDrawing(state).then((result) => {
      if (cancelled) return;
      if (result.status === "ok") {
        setDrawing(result.data);
      } else {
        // Storage/Internal — render the empty fallback rather than crash.
        // eslint-disable-next-line no-console
        console.warn("editor_load_drawing failed", result.error);
        setDrawing(null);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [state]);

  return drawing;
}
