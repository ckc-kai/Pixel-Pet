/**
 * Tool panel — brush / eraser / undo.
 *
 * Eraser is implemented as "paint with the transparent palette slot" — it
 * does not need its own paint path. The brush remembers the last non-eraser
 * color so toggling back from eraser feels right.
 */

export type Tool = "brush" | "eraser";

interface ToolPanelProps {
  readonly tool: Tool;
  readonly canUndo: boolean;
  readonly onToolChange: (tool: Tool) => void;
  readonly onUndo: () => void;
}

export function ToolPanel({
  tool,
  canUndo,
  onToolChange,
  onUndo,
}: ToolPanelProps) {
  return (
    <div className="rail-group">
      <div className="rail-label">Tools</div>
      <div className="tool-row">
        <button
          type="button"
          className={`tool-button${tool === "brush" ? " tool-button--active" : ""}`}
          aria-pressed={tool === "brush"}
          onClick={() => onToolChange("brush")}
        >
          Brush
        </button>
        <button
          type="button"
          className={`tool-button${tool === "eraser" ? " tool-button--active" : ""}`}
          aria-pressed={tool === "eraser"}
          onClick={() => onToolChange("eraser")}
        >
          Eraser
        </button>
      </div>
      <button
        type="button"
        className="action-button"
        disabled={!canUndo}
        onClick={onUndo}
      >
        Undo
      </button>
    </div>
  );
}
