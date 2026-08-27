/**
 * The vertical document list.
 *
 * Open documents are cards stacked under each other, the way vertical tabs work
 * in Zen. The sidebar can be collapsed entirely and comes back when the pointer
 * approaches the left edge.
 */

import type { DocumentSummary } from "@/lib/types";
import { formatBytes, formatPages } from "@/lib/format";
import { IconClose, IconDocument, IconPlus } from "./Icons";

interface Props {
  documents: DocumentSummary[];
  activeId: number | null;
  collapsed: boolean;
  onSelect: (id: number) => void;
  onClose: (id: number) => void;
  onOpen: () => void;
}

export function Sidebar({ documents, activeId, collapsed, onSelect, onClose, onOpen }: Props) {
  return (
    <aside className={`sidebar panel${collapsed ? " sidebar--collapsed" : ""}`} aria-label="Geoeffnete Dokumente">
      <div className="sidebar__head">
        <span className="sidebar__title">Dokumente</span>
        <button className="icon-button" onClick={onOpen} title="PDF oeffnen (Strg+O)">
          <IconPlus size={16} />
          <span className="visually-hidden">PDF oeffnen</span>
        </button>
      </div>

      <div className="sidebar__list">
        {documents.length === 0 && (
          <p className="sidebar__empty">
            Noch nichts geoeffnet. Zieh ein PDF hierher oder druecke Strg und O.
          </p>
        )}

        {documents.map((doc) => (
          <button
            key={doc.id}
            className={`doc-card${doc.id === activeId ? " doc-card--active" : ""}`}
            onClick={() => onSelect(doc.id)}
          >
            <span className="doc-card__icon">
              <IconDocument size={16} />
            </span>
            <span className="doc-card__body">
              <span className="doc-card__name" title={doc.source.displayName}>
                {doc.source.displayName}
              </span>
              <span className="doc-card__meta">
                {formatPages(doc.pageCount)} · {formatBytes(doc.byteSize)}
              </span>
            </span>
            {doc.dirty && <span className="doc-card__dot" title="Nicht gespeicherte Aenderungen" />}
            <span
              className="doc-card__close"
              role="button"
              tabIndex={-1}
              title="Schliessen"
              onClick={(event) => {
                event.stopPropagation();
                onClose(doc.id);
              }}
            >
              <IconClose size={13} />
            </span>
          </button>
        ))}
      </div>
    </aside>
  );
}
