/**
 * The properties panel on the right.
 *
 * It is only there when something is selected. In M0 the selection is the open
 * document, so it shows what the core knows about the file. From M5 on it also
 * carries the properties of the selected object.
 */

import type { CoreInfo, DocumentSummary } from "@/lib/types";
import { formatBytes } from "@/lib/format";

interface Props {
  document: DocumentSummary;
  info: CoreInfo | null;
  pageIndex: number;
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="row">
      <span className="row__label">{label}</span>
      <span className="row__value">{value}</span>
    </div>
  );
}

export function Inspector({ document: doc, info, pageIndex }: Props) {
  const page = doc.pages[pageIndex];

  return (
    <aside className="inspector panel" aria-label="Eigenschaften">
      <h2 className="inspector__title">Dokument</h2>
      <Row label="Name" value={doc.source.displayName} />
      <Row label="PDF-Version" value={doc.pdfVersion || "unbekannt"} />
      <Row label="Seiten" value={String(doc.pageCount)} />
      <Row label="Groesse" value={formatBytes(doc.byteSize)} />
      <Row label="Verschluesselt" value={doc.wasEncrypted ? "ja, entschluesselt" : "nein"} />
      <Row label="Geaendert" value={doc.dirty ? "ja, nicht gespeichert" : "nein"} />

      {page && (
        <>
          <h2 className="inspector__title">Seite {page.index + 1}</h2>
          <Row
            label="Groesse"
            value={`${page.widthPt.toFixed(1)} auf ${page.heightPt.toFixed(1)} pt`}
          />
          <Row label="Drehung" value={`${page.rotation} Grad`} />
          <Row label="Inhaltsstroeme" value={String(page.contentStreamCount)} />
          <Row label="Anmerkungen" value={String(page.annotationCount)} />
          <Row label="Objekt" value={`${page.object.number} ${page.object.generation}`} />
        </>
      )}

      {doc.history.undo.length > 0 && (
        <>
          <h2 className="inspector__title">Verlauf</h2>
          <ol className="history">
            {doc.history.undo.slice(0, 8).map((entry, index) => (
              <li key={`${entry.label}-${index}`} className="history__item">
                {entry.label}
              </li>
            ))}
          </ol>
        </>
      )}

      {info && (
        <>
          <h2 className="inspector__title">Kern</h2>
          <Row label="Version" value={info.version} />
          <Row label="Plattform" value={info.platform} />
          <Row
            label="Renderer"
            value={info.renderer.available ? info.renderer.backend : "nicht geladen"}
          />
        </>
      )}
    </aside>
  );
}
