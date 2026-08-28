/**
 * The floating tool bar.
 *
 * There is no permanent ribbon. This bar appears when a document is open and
 * carries only what applies to the current selection. On a touch layout it
 * moves to the bottom edge, into thumb reach, which the stylesheet does through
 * the same breakpoints the rest of the app uses.
 */

import type { DocumentSummary } from "@/lib/types";
import { IconRedo, IconRotate, IconSave, IconUndo } from "./Icons";

interface Props {
  document: DocumentSummary;
  busy: boolean;
  zoom: number;
  onZoom: (zoom: number) => void;
  onRotate: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
}

export function FloatingToolbar({
  document: doc,
  busy,
  zoom,
  onZoom,
  onRotate,
  onUndo,
  onRedo,
  onSave,
}: Props) {
  return (
    <div className="toolbar panel" role="toolbar" aria-label="Werkzeuge">
      <button className="tool" onClick={onRotate} disabled={busy} title="Seite drehen">
        <IconRotate size={17} />
      </button>

      <span className="toolbar__divider" />

      <button
        className="tool"
        onClick={onUndo}
        disabled={busy || !doc.history.canUndo}
        title={
          doc.history.undo[0] ? `Rueckgaengig: ${doc.history.undo[0].label}` : "Rueckgaengig"
        }
      >
        <IconUndo size={17} />
      </button>
      <button
        className="tool"
        onClick={onRedo}
        disabled={busy || !doc.history.canRedo}
        title={
          doc.history.redo[0] ? `Wiederholen: ${doc.history.redo[0].label}` : "Wiederholen"
        }
      >
        <IconRedo size={17} />
      </button>

      <span className="toolbar__divider" />

      <div className="zoom">
        <button className="tool tool--tight" onClick={() => onZoom(zoom / 1.25)} title="Kleiner">
          &minus;
        </button>
        <span className="zoom__value">{Math.round(zoom * 100)} %</span>
        <button className="tool tool--tight" onClick={() => onZoom(zoom * 1.25)} title="Groesser">
          +
        </button>
      </div>

      <span className="toolbar__divider" />

      <button
        className="tool tool--primary"
        onClick={onSave}
        disabled={busy || !doc.dirty}
        title="Speichern (Strg+S)"
      >
        <IconSave size={17} />
      </button>
    </div>
  );
}
