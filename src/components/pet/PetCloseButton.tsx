// Debug-only "quit" button for the pet window.
//
// Renders ONLY in dev builds (`import.meta.env.DEV`). Production users
// will quit via the tray menu / settings popover (Branch 2). Until that
// lands, this saves us from having to Ctrl+C the dev server every time.
//
// Behavior:
//   - Calls `getCurrentWindow().destroy()`. On macOS the Tauri app exits
//     once the last visible window is destroyed; if it doesn't (depends
//     on plugin config), Ctrl+C in the terminal still works as a fallback.
//
// Visual:
//   - Top-right corner of the pet window
//   - Tiny dark glass pill matching the dev state panel
//   - Sized so it doesn't smother the pet at 96×96

import { getCurrentWindow } from "@tauri-apps/api/window";

export function PetCloseButton() {
  if (!import.meta.env.DEV) return null;

  const close = () => {
    getCurrentWindow()
      .destroy()
      .catch((err) => {
        // eslint-disable-next-line no-console
        console.warn("pet window destroy failed", err);
      });
  };

  return (
    <button
      type="button"
      className="pet-close-button"
      onClick={close}
      title="Quit pet (dev only — Ctrl+C in terminal also works)"
      aria-label="Quit pet window"
    >
      ×
    </button>
  );
}
