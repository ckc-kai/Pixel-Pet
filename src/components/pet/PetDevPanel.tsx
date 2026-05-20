// Debug-only floating panel for forcing pet-state transitions.
//
// Renders ONLY in dev builds:
//   - The component returns `null` at runtime when `import.meta.env.DEV`
//     is false.
//   - The `pet_force_transition` IPC command is itself compiled out of
//     release builds (`#[cfg(debug_assertions)]` in src-tauri/src/ipc/pet.rs),
//     so the panel could never call it in production even if it leaked in.

import type { PetState } from "../../lib/types/bindings";
import { commands } from "../../lib/types/bindings";

interface PetDevPanelProps {
  current: PetState | null;
}

// Listed explicitly so the dev panel never silently drifts when a new
// PetState is added — TS will flag the omission at the consumer site.
const STATES: readonly PetState[] = [
  "startup",
  "working",
  "stretch",
  "tired",
  "sleep",
  "spaced_out",
  "eating",
];

export function PetDevPanel({ current }: PetDevPanelProps) {
  if (!import.meta.env.DEV) return null;

  const force = (target: PetState) => {
    commands.petForceTransition(target).then((result) => {
      if (result.status === "error") {
        // eslint-disable-next-line no-console
        console.warn("pet_force_transition failed", result.error);
      }
    });
  };

  return (
    <div className="pet-dev-panel" role="group" aria-label="Force pet state">
      {STATES.map((s) => (
        <button
          key={s}
          type="button"
          onClick={() => force(s)}
          aria-current={current === s}
        >
          {s}
        </button>
      ))}
    </div>
  );
}
