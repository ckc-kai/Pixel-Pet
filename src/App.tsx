/**
 * Root component. Both Tauri windows (`editor`, `pet`) load the same JS
 * bundle, so we route by the current webview window label.
 *
 * - `editor` label → one-time pixel editor (this PR).
 * - `pet` label    → PR 1.3 owns the surface in `src/components/pet/*`.
 *                   Until that lands, render a transparent placeholder so the
 *                   pet window isn't a white rectangle on first dual-window
 *                   bring-up.
 */

import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { EditorRoot } from "./components/editor/EditorRoot";
import "./styles/tokens.css";
import "./styles/editor.css";

const EDITOR_WINDOW_LABEL = "editor";

function resolveWindowLabel(): string {
  try {
    return getCurrentWebviewWindow().label;
  } catch {
    // Running outside Tauri (e.g. plain `vite dev` for component tinkering).
    // Default to the editor surface — it's the only thing this PR provides.
    return EDITOR_WINDOW_LABEL;
  }
}

const WINDOW_LABEL = resolveWindowLabel();

function App() {
  if (WINDOW_LABEL === EDITOR_WINDOW_LABEL) {
    return <EditorRoot />;
  }

  // Pet window placeholder. PR 1.3 will replace this branch with the real
  // <PetSurface /> import. Transparent so it composes with the borderless
  // always-on-top pet window declared in tauri.conf.json.
  return <div aria-hidden="true" style={{ background: "transparent" }} />;
}

export default App;
