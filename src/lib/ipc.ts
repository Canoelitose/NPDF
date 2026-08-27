/**
 * Typed access to the Rust core.
 *
 * Nothing else in the frontend calls `invoke` directly. That keeps the command
 * names in one place and gives every call a real type.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  CommandError,
  CoreInfo,
  DocumentHandle,
  DocumentSummary,
  EditCommand,
  HistoryEntry,
  PageText,
  RenderRequest,
  RenderedPage,
  SaveMode,
  SaveReport,
} from "./types";

/** Header layout of the render reply, see `commands/dto.rs`. */
const RENDER_HEADER_BYTES = 20;

export class NpdfError extends Error {
  readonly code: string;

  constructor(error: CommandError) {
    super(error.message);
    this.name = "NpdfError";
    this.code = error.code;
  }
}

function toNpdfError(error: unknown): NpdfError {
  if (error && typeof error === "object" && "code" in error && "message" in error) {
    return new NpdfError(error as CommandError);
  }
  return new NpdfError({ code: "io", message: String(error) });
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toNpdfError(error);
  }
}

export const coreInfo = () => call<CoreInfo>("core_info");

export const openDocumentPath = (path: string, password?: string) =>
  call<DocumentSummary>("open_document_path", { path, password });

export const openDocumentBytes = (
  name: string,
  bytes: Uint8Array,
  handle?: DocumentHandle,
  password?: string,
) =>
  call<DocumentSummary>("open_document_bytes", {
    name,
    bytes: Array.from(bytes),
    handle,
    password,
  });

export const closeDocument = (id: number) => call<void>("close_document", { id });

export const listDocuments = () => call<DocumentSummary[]>("list_documents");

export const documentSummary = (id: number) => call<DocumentSummary>("document_summary", { id });

export const pageText = (id: number, pageIndex: number) =>
  call<PageText>("page_text", { id, pageIndex });

export const applyEdit = (id: number, command: EditCommand) =>
  call<HistoryEntry>("apply_edit", { id, command });

export const undo = (id: number) => call<HistoryEntry>("undo", { id });

export const redo = (id: number) => call<HistoryEntry>("redo", { id });

export const saveDocument = (id: number, target?: DocumentHandle, mode: SaveMode = "incremental") =>
  call<SaveReport>("save_document", { id, target, mode });

export const releaseMemory = () => call<void>("release_memory");

export const restoreMemoryBudget = () => call<void>("restore_memory_budget");

/**
 * Render a page. The reply is a binary buffer rather than JSON, because a page
 * bitmap is measured in megabytes.
 */
export async function renderPage(id: number, request: RenderRequest): Promise<RenderedPage> {
  const reply = await call<ArrayBuffer>("render_page", { id, request });
  const buffer: ArrayBuffer =
    reply instanceof ArrayBuffer ? reply : new Uint8Array(reply).slice().buffer;
  const view = new DataView(buffer);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  const scale = view.getFloat32(8, true);
  const pageIndex = view.getUint32(12, true);
  const length = view.getUint32(16, true);
  const pixels = new Uint8ClampedArray(buffer, RENDER_HEADER_BYTES, length);
  return { pageIndex, width, height, scale, pixels };
}

/**
 * Save and get the bytes back instead of writing them. This is the path mobile
 * uses, where only the platform document API may write.
 */
export async function saveDocumentBytes(
  id: number,
  mode: SaveMode = "incremental",
): Promise<{ report: SaveReport; bytes: Uint8Array }> {
  const reply = await call<ArrayBuffer>("save_document_bytes", { id, mode });
  const buffer: ArrayBuffer =
    reply instanceof ArrayBuffer ? reply : new Uint8Array(reply).slice().buffer;
  const view = new DataView(buffer);
  const reportLength = view.getUint32(0, true);
  const reportJson = new TextDecoder().decode(new Uint8Array(buffer, 4, reportLength));
  return {
    report: JSON.parse(reportJson) as SaveReport,
    bytes: new Uint8Array(buffer, 4 + reportLength),
  };
}
