/**
 * The page itself, floating in the middle of the window.
 *
 * When the renderer is there the page is drawn into a canvas. When it is not,
 * the same floating surface shows the real page geometry and says plainly what
 * is missing, instead of leaving an empty rectangle.
 */

import { useEffect, useRef, useState } from "react";

import * as ipc from "@/lib/ipc";
import type { CoreInfo, DocumentSummary, PageInfo } from "@/lib/types";

interface Props {
  document: DocumentSummary | null;
  info: CoreInfo | null;
  pageIndex: number;
  zoom: number;
}

export function PageStage({ document: doc, info, pageIndex, zoom }: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const page: PageInfo | undefined = doc?.pages[pageIndex];

  useEffect(() => {
    let cancelled = false;
    const canvas = canvasRef.current;
    if (!doc || !page || !canvas || !info?.renderer.available) return;

    const ratio = window.devicePixelRatio || 1;
    ipc
      .renderPage(doc.id, { pageIndex, scale: zoom * ratio })
      .then((rendered) => {
        if (cancelled) return;
        canvas.width = rendered.width;
        canvas.height = rendered.height;
        canvas.style.width = `${rendered.width / ratio}px`;
        canvas.style.height = `${rendered.height / ratio}px`;
        const context = canvas.getContext("2d");
        if (!context) return;
        context.putImageData(
          new ImageData(rendered.pixels, rendered.width, rendered.height),
          0,
          0,
        );
        setFailure(null);
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(String(error));
      });

    return () => {
      cancelled = true;
    };
    // The revision changes on every edit, which is what forces a redraw.
  }, [doc?.id, doc?.revision, page, pageIndex, zoom, info?.renderer.available]);

  if (!doc || !page) {
    return (
      <div className="stage stage--empty">
        <div className="stage__welcome panel">
          <h1>NPDF</h1>
          <p>
            Ein PDF-Editor, der vorhandenen Text wirklich aendert statt ihn zu ueberlagern.
            Alles laeuft lokal, ohne Konto und ohne Cloud.
          </p>
          <p className="stage__hint">
            Strg und O oeffnet eine Datei. Strg und K oeffnet die Kommandopalette.
          </p>
        </div>
      </div>
    );
  }

  const showCanvas = info?.renderer.available && !failure;
  const width = page.widthPt * zoom;
  const height = page.heightPt * zoom;

  return (
    <div className="stage">
      <div className="page" style={{ width: `${width}px`, minHeight: `${height}px` }}>
        {showCanvas ? (
          <canvas ref={canvasRef} className="page__canvas" />
        ) : (
          <div className="page__placeholder" style={{ height: `${height}px` }}>
            <span className="page__placeholder-title">Seite {page.index + 1}</span>
            <span className="page__placeholder-meta">
              {page.widthPt.toFixed(0)} auf {page.heightPt.toFixed(0)} Punkt
              {page.rotation !== 0 && `, ${page.rotation} Grad gedreht`}
            </span>
            <p className="page__placeholder-note">
              {failure ?? info?.renderer.detail ?? "Die Seitendarstellung steht nicht bereit."}
            </p>
          </div>
        )}
      </div>
      <div className="stage__caption">
        Seite {page.index + 1} von {doc.pageCount}
      </div>
    </div>
  );
}
