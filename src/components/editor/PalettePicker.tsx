/**
 * Palette picker — 16 swatches in a 4×4 grid. Slot 0 is the transparent /
 * eraser slot and renders with a crosshatch instead of a hex fill.
 */

import {
  DRAWING_PALETTE,
  TRANSPARENT_INDEX,
  paletteCssVar,
} from "./palette";

interface PalettePickerProps {
  readonly activeIndex: number;
  readonly onSelect: (index: number) => void;
}

export function PalettePicker({ activeIndex, onSelect }: PalettePickerProps) {
  return (
    <div className="rail-group">
      <div className="rail-label">Palette</div>
      <div className="palette-grid" role="radiogroup" aria-label="Color palette">
        {DRAWING_PALETTE.map((_hex, index) => {
          const isActive = index === activeIndex;
          const isTransparent = index === TRANSPARENT_INDEX;
          const swatchStyle = {
            // The transparent swatch uses its own background pattern from CSS.
            ["--swatch-color" as string]: `var(${paletteCssVar(index)})`,
          } as React.CSSProperties;
          const className = [
            "palette-swatch",
            isTransparent ? "palette-swatch--transparent" : "",
            isActive ? "palette-swatch--active" : "",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <button
              key={index}
              type="button"
              role="radio"
              aria-checked={isActive}
              aria-label={
                isTransparent
                  ? "Transparent (eraser color)"
                  : `Color ${index}`
              }
              className={className}
              style={swatchStyle}
              onClick={() => onSelect(index)}
            />
          );
        })}
      </div>
    </div>
  );
}
