// Entry for the dedicated pet window.
//
// TODO(integration): tauri.conf.json currently has no `url` set on the pet
// window, so Tauri loads `index.html` in both windows by default. PR 1.2
// owns App.tsx / index.html and the wider window-routing story, so we
// can't flip that switch here. Follow-up: set
//   "url": "pet.html"
// on the pet window in src-tauri/tauri.conf.json (and add it to the
// allowed Vite build inputs — done in vite.config.ts). Until that lands,
// the pet window will render whatever index.html mounts (currently PR 1.1's
// editor), and this entry only loads when a window explicitly requests
// pet.html.

import React from "react";
import ReactDOM from "react-dom/client";
import { PetRenderer } from "./components/pet/PetRenderer";

const root = document.getElementById("root");
if (!root) {
  throw new Error("pet window: #root not found");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <PetRenderer />
  </React.StrictMode>,
);
