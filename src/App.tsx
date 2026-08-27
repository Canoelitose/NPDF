import { useCallback, useEffect, useMemo, useState } from "react";

import { CommandPalette, type Command } from "@/components/CommandPalette";
import { FloatingToolbar } from "@/components/FloatingToolbar";
import { Inspector } from "@/components/Inspector";
import { PageStage } from "@/components/PageStage";
import { Sidebar } from "@/components/Sidebar";
import { IconCommand, IconSidebar, IconSun } from "@/components/Icons";
import { applyAccent, applyTheme, loadAccent, loadTheme, type ThemePreference } from "@/lib/theme";
import { useSession } from "@/state/useSession";

const ZOOM_MIN = 0.2;
const ZOOM_MAX = 6;

export default function App() {
  const session = useSession();
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [zoom, setZoom] = useState(1);
  const [theme, setTheme] = useState<ThemePreference>(loadTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    applyAccent(loadAccent());
  }, []);

  const setZoomClamped = useCallback((next: number) => {
    setZoom(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next)));
  }, []);

  const rotate = useCallback(() => {
    const page = session.active?.pages[session.pageIndex];
    if (!page) return;
    void session.applyEdit({
      type: "rotatePage",
      pageIndex: page.index,
      degrees: (page.rotation + 90) % 360,
    });
  }, [session]);

  const commands = useMemo<Command[]>(() => {
    const hasDocument = session.active !== null;
    return [
      { id: "open", label: "PDF oeffnen", hint: "Strg O", enabled: true, run: () => void session.openDocument() },
      {
        id: "save",
        label: "Speichern",
        hint: "Strg S",
        enabled: hasDocument,
        run: () => void session.save(false),
      },
      {
        id: "save-as",
        label: "Speichern unter",
        enabled: hasDocument,
        run: () => void session.save(true),
      },
      { id: "rotate", label: "Seite drehen", enabled: hasDocument, run: rotate },
      {
        id: "undo",
        label: "Rueckgaengig",
        hint: "Strg Z",
        enabled: session.active?.history.canUndo ?? false,
        run: () => void session.undo(),
      },
      {
        id: "redo",
        label: "Wiederholen",
        hint: "Strg Y",
        enabled: session.active?.history.canRedo ?? false,
        run: () => void session.redo(),
      },
      {
        id: "sidebar",
        label: sidebarCollapsed ? "Seitenleiste einblenden" : "Seitenleiste ausblenden",
        hint: "Strg B",
        enabled: true,
        run: () => setSidebarCollapsed((value) => !value),
      },
      {
        id: "theme",
        label: "Helles und dunkles Erscheinungsbild wechseln",
        enabled: true,
        run: () => setTheme((current) => (current === "light" ? "dark" : "light")),
      },
      { id: "zoom-reset", label: "Zoom auf 100 Prozent", enabled: hasDocument, run: () => setZoom(1) },
    ];
  }, [rotate, session, sidebarCollapsed]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const modifier = event.ctrlKey || event.metaKey;
      if (modifier && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }
      if (!modifier) return;
      const key = event.key.toLowerCase();
      if (key === "o") {
        event.preventDefault();
        void session.openDocument();
      } else if (key === "s") {
        event.preventDefault();
        void session.save(event.shiftKey);
      } else if (key === "b") {
        event.preventDefault();
        setSidebarCollapsed((value) => !value);
      } else if (key === "z") {
        event.preventDefault();
        void (event.shiftKey ? session.redo() : session.undo());
      } else if (key === "y") {
        event.preventDefault();
        void session.redo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [session]);

  const active = session.active;

  return (
    <div className={`shell${sidebarCollapsed ? " shell--compact" : ""}`}>
      <div className="titlebar" data-tauri-drag-region>
        <button
          className="icon-button"
          onClick={() => setSidebarCollapsed((value) => !value)}
          title="Seitenleiste (Strg+B)"
        >
          <IconSidebar size={16} />
        </button>
        <span className="titlebar__name">{active?.source.displayName ?? "NPDF"}</span>
        {active?.dirty && <span className="titlebar__badge">nicht gespeichert</span>}
        <span className="titlebar__spacer" />
        <button className="icon-button" onClick={() => setPaletteOpen(true)} title="Kommandopalette (Strg+K)">
          <IconCommand size={16} />
        </button>
        <button
          className="icon-button"
          onClick={() => setTheme((current) => (current === "light" ? "dark" : "light"))}
          title="Erscheinungsbild wechseln"
        >
          <IconSun size={16} />
        </button>
      </div>

      <Sidebar
        documents={session.documents}
        activeId={session.activeId}
        collapsed={sidebarCollapsed}
        onSelect={(id) => {
          session.setActiveId(id);
          session.setPageIndex(0);
        }}
        onClose={(id) => void session.closeDocument(id)}
        onOpen={() => void session.openDocument()}
      />

      <main className="main">
        <PageStage
          document={active}
          info={session.info}
          pageIndex={session.pageIndex}
          zoom={zoom}
        />

        {active && active.pageCount > 1 && (
          <div className="pager panel">
            <button
              className="tool tool--tight"
              disabled={session.pageIndex === 0}
              onClick={() => session.setPageIndex(session.pageIndex - 1)}
            >
              &lsaquo;
            </button>
            <span className="pager__value">
              {session.pageIndex + 1} / {active.pageCount}
            </span>
            <button
              className="tool tool--tight"
              disabled={session.pageIndex >= active.pageCount - 1}
              onClick={() => session.setPageIndex(session.pageIndex + 1)}
            >
              &rsaquo;
            </button>
          </div>
        )}

        {active && (
          <FloatingToolbar
            document={active}
            busy={session.busy}
            zoom={zoom}
            onZoom={setZoomClamped}
            onRotate={rotate}
            onUndo={() => void session.undo()}
            onRedo={() => void session.redo()}
            onSave={() => void session.save(false)}
          />
        )}
      </main>

      {active && <Inspector document={active} info={session.info} pageIndex={session.pageIndex} />}

      {session.notice && (
        <div className={`notice notice--${session.notice.tone}`} onClick={session.dismissNotice}>
          {session.notice.text}
        </div>
      )}

      <CommandPalette
        open={paletteOpen}
        commands={commands}
        onClose={() => setPaletteOpen(false)}
      />
    </div>
  );
}
