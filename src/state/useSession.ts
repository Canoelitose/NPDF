/**
 * The one place that talks to the core and holds what the screen shows.
 *
 * Deliberately a plain hook with plain state. There is no store library, because
 * the real document model lives in Rust and this side only mirrors a summary.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";

import * as ipc from "@/lib/ipc";
import { NpdfError } from "@/lib/ipc";
import { germanMessage } from "@/lib/format";
import type { CoreInfo, DocumentSummary, EditCommand } from "@/lib/types";

export interface Notice {
  tone: "info" | "error";
  text: string;
}

export function useSession() {
  const [info, setInfo] = useState<CoreInfo | null>(null);
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [activeId, setActiveId] = useState<number | null>(null);
  const [pageIndex, setPageIndex] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const noticeTimer = useRef<number | null>(null);

  const active = documents.find((doc) => doc.id === activeId) ?? null;

  const say = useCallback((next: Notice | null) => {
    setNotice(next);
    if (noticeTimer.current !== null) window.clearTimeout(noticeTimer.current);
    if (next) {
      noticeTimer.current = window.setTimeout(() => setNotice(null), 6000);
    }
  }, []);

  const report = useCallback(
    (error: unknown) => {
      if (error instanceof NpdfError) {
        say({ tone: "error", text: germanMessage(error.code, error.message) });
      } else {
        say({ tone: "error", text: String(error) });
      }
    },
    [say],
  );

  useEffect(() => {
    ipc.coreInfo().then(setInfo).catch(report);
  }, [report]);

  // Give memory back when the app is hidden. On a phone this decides whether the
  // system keeps the app alive or kills it.
  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "hidden") {
        ipc.releaseMemory().catch(() => undefined);
      } else {
        ipc.restoreMemoryBudget().catch(() => undefined);
      }
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  const refresh = useCallback(async () => {
    setDocuments(await ipc.listDocuments());
  }, []);

  const openDocument = useCallback(async () => {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : String(selected);

    setBusy(true);
    try {
      let summary: DocumentSummary;
      if (info?.capabilities.freeFilePaths) {
        summary = await ipc.openDocumentPath(path);
      } else {
        // On iOS and Android the picker hands back a handle the file system
        // plugin can resolve, never a path the core could open itself.
        const bytes = await readFile(path);
        const name = path.split(/[/\\]/).pop() ?? "Dokument.pdf";
        summary = await ipc.openDocumentBytes(name, bytes);
      }
      await refresh();
      setActiveId(summary.id);
      setPageIndex(0);
      say({ tone: "info", text: `${summary.source.displayName} geoeffnet` });
    } catch (error) {
      report(error);
    } finally {
      setBusy(false);
    }
  }, [info, refresh, report, say]);

  const closeDocument = useCallback(
    async (id: number) => {
      try {
        await ipc.closeDocument(id);
        await refresh();
        setActiveId((current) => (current === id ? null : current));
      } catch (error) {
        report(error);
      }
    },
    [refresh, report],
  );

  const run = useCallback(
    async (action: () => Promise<unknown>, success?: string) => {
      setBusy(true);
      try {
        await action();
        await refresh();
        if (success) say({ tone: "info", text: success });
      } catch (error) {
        report(error);
      } finally {
        setBusy(false);
      }
    },
    [refresh, report, say],
  );

  const applyEdit = useCallback(
    (command: EditCommand) => {
      if (activeId === null) return Promise.resolve();
      return run(async () => {
        const entry = await ipc.applyEdit(activeId, command);
        say({ tone: "info", text: entry.label });
      });
    },
    [activeId, run, say],
  );

  const undo = useCallback(() => {
    if (activeId === null) return Promise.resolve();
    return run(async () => {
      const entry = await ipc.undo(activeId);
      say({ tone: "info", text: `Rueckgaengig: ${entry.label}` });
    });
  }, [activeId, run, say]);

  const redo = useCallback(() => {
    if (activeId === null) return Promise.resolve();
    return run(async () => {
      const entry = await ipc.redo(activeId);
      say({ tone: "info", text: `Wiederhergestellt: ${entry.label}` });
    });
  }, [activeId, run, say]);

  const save = useCallback(
    (as: boolean) => {
      if (activeId === null || !active) return Promise.resolve();
      return run(async () => {
        if (info?.capabilities.freeFilePaths) {
          let target: string | null = null;
          if (as || !active.source.handle) {
            target = await saveFileDialog({
              defaultPath: active.source.displayName,
              filters: [{ name: "PDF", extensions: ["pdf"] }],
            });
            if (!target) return;
          }
          const report_ = await ipc.saveDocument(
            activeId,
            target ? { kind: "path", value: target } : undefined,
          );
          say({
            tone: "info",
            text: `Gespeichert, ${report_.changedObjects} Objekte neu geschrieben, Pruefung bestanden`,
          });
        } else {
          // Mobile writes through the platform, the core only hands bytes over.
          const target = await saveFileDialog({ defaultPath: active.source.displayName });
          if (!target) return;
          const { report: saveReport, bytes } = await ipc.saveDocumentBytes(activeId);
          await writeFile(target, bytes);
          say({
            tone: "info",
            text: `Gespeichert, ${saveReport.changedObjects} Objekte neu geschrieben`,
          });
        }
      });
    },
    [active, activeId, info, run, say],
  );

  return {
    info,
    documents,
    active,
    activeId,
    setActiveId,
    pageIndex,
    setPageIndex,
    busy,
    notice,
    dismissNotice: () => say(null),
    openDocument,
    closeDocument,
    applyEdit,
    undo,
    redo,
    save,
  };
}

export type SessionApi = ReturnType<typeof useSession>;
