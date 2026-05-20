/**
 * Top-level editor surface. Owns:
 * - active tool + active color
 * - drawing state (delegated to `useDrawingState`)
 * - confirm ritual (warning text + irreversible IPC sequence + fade-out)
 */

import { useCallback, useMemo, useState } from "react";

import { commands, type Drawing, type IpcError } from "../../lib/types/bindings";

import {
  CANVAS_HEIGHT,
  CANVAS_WIDTH,
  DRAWING_SCHEMA_VERSION,
  EDITOR_TARGET_STATE,
} from "./constants";
import { PalettePicker } from "./PalettePicker";
import { PixelCanvas } from "./PixelCanvas";
import { DRAWING_PALETTE, TRANSPARENT_INDEX } from "./palette";
import { ToolPanel, type Tool } from "./ToolPanel";
import { useDrawingState } from "./useDrawingState";

type Phase = "drawing" | "saving" | "fading";

function ipcErrorMessage(err: IpcError): string {
  switch (err.kind) {
    case "BadRequest":
      return err.message;
    case "NotFound":
      return "Required resource was not found.";
    case "Storage":
      return "Could not save your drawing to disk.";
    case "Internal":
      return "Something went wrong inside the app.";
  }
}

export function EditorRoot() {
  const { pixels, canUndo, beginStroke, paintPixel, endStroke, undo } =
    useDrawingState({ width: CANVAS_WIDTH, height: CANVAS_HEIGHT });

  // Brush color and tool are two pieces of state so toggling Brush↔Eraser
  // doesn't forget the user's previous color choice.
  const [activeColor, setActiveColor] = useState<number>(1);
  const [tool, setTool] = useState<Tool>("brush");
  const [phase, setPhase] = useState<Phase>("drawing");
  const [errorText, setErrorText] = useState<string | null>(null);

  const paintIndex = tool === "eraser" ? TRANSPARENT_INDEX : activeColor;

  const handleStrokePaint = useCallback(
    (x: number, y: number) => {
      paintPixel(x, y, paintIndex);
    },
    [paintPixel, paintIndex],
  );

  const handleColorSelect = useCallback((index: number) => {
    setActiveColor(index);
    // Picking any color implies brush mode — eraser stays explicit.
    if (index !== TRANSPARENT_INDEX) {
      setTool("brush");
    } else {
      setTool("eraser");
    }
  }, []);

  const handleToolChange = useCallback((next: Tool) => {
    setTool(next);
  }, []);

  const hasAnyPaintedPixel = useMemo(
    () => pixels.some((row) => row.some((cell) => cell !== TRANSPARENT_INDEX)),
    [pixels],
  );

  const handleConfirm = useCallback(async (): Promise<void> => {
    if (phase !== "drawing") return;
    setErrorText(null);
    setPhase("saving");

    const drawing: Drawing = {
      schema_version: DRAWING_SCHEMA_VERSION,
      state: EDITOR_TARGET_STATE,
      width: CANVAS_WIDTH,
      height: CANVAS_HEIGHT,
      palette: [...DRAWING_PALETTE],
      pixels: pixels.map((row) => [...row]),
    };

    try {
      const saveResult = await commands.editorSaveDrawing(
        EDITOR_TARGET_STATE,
        drawing,
      );
      if (saveResult.status === "error") {
        setErrorText(ipcErrorMessage(saveResult.error));
        setPhase("drawing");
        return;
      }

      const completeResult = await commands.systemCompleteDrawing();
      if (completeResult.status === "error") {
        setErrorText(ipcErrorMessage(completeResult.error));
        setPhase("drawing");
        return;
      }

      // Backend will hide the editor window. Run a short fade-out so the
      // transition feels intentional rather than abrupt.
      setPhase("fading");
    } catch (err) {
      // Non-IPC throw (transport panic). Surface a generic message and let
      // the user retry — the backend's window swap is the source of truth.
      setErrorText("Could not finish the drawing ritual. Try again.");
      setPhase("drawing");
      // eslint-disable-next-line no-console
      void err;
    }
  }, [phase, pixels]);

  const shellClassName = `editor-shell${
    phase === "fading" ? " editor-shell--fading" : ""
  }`;

  return (
    <main className={shellClassName} aria-busy={phase === "saving"}>
      <header className="editor-header">
        <h1 className="editor-title">Draw Your Pet</h1>
        <p className="editor-subtitle">
          64 × 64 pixels · 16 colors · one-time ritual
        </p>
        <p className="editor-tip">
          Tip: draw a <strong>head AND body</strong> for the best animations.
        </p>
      </header>

      <section className="editor-body">
        <PixelCanvas
          pixels={pixels}
          width={CANVAS_WIDTH}
          height={CANVAS_HEIGHT}
          onStrokeBegin={beginStroke}
          onStrokePaint={handleStrokePaint}
          onStrokeEnd={endStroke}
        />
        <aside className="rail">
          <PalettePicker
            activeIndex={tool === "eraser" ? TRANSPARENT_INDEX : activeColor}
            onSelect={handleColorSelect}
          />
          <ToolPanel
            tool={tool}
            canUndo={canUndo && phase === "drawing"}
            onToolChange={handleToolChange}
            onUndo={undo}
          />
        </aside>
      </section>

      <section className="confirm-zone">
        <p className="confirm-warning">
          Once you confirm, this drawing cannot be changed.
        </p>
        <button
          type="button"
          className="confirm-button"
          disabled={phase !== "drawing" || !hasAnyPaintedPixel}
          onClick={() => {
            void handleConfirm();
          }}
        >
          {phase === "saving" ? "Saving…" : "Confirm Drawing"}
        </button>
        <div
          className={`status-line${errorText ? " status-line--error" : ""}`}
          role="status"
          aria-live="polite"
        >
          {errorText ??
            (!hasAnyPaintedPixel ? "Paint at least one pixel to continue." : "")}
        </div>
      </section>
    </main>
  );
}
