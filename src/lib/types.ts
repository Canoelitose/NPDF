/**
 * The shapes that cross the bridge from Rust.
 *
 * These mirror the serde definitions in `npdf-core`. They are written by hand on
 * purpose for now, and the Rust side has tests that pin the wire format so a
 * rename cannot drift silently. Generating them belongs in a later milestone.
 */

export type PlatformKind = "windows" | "macos" | "linux" | "ios" | "android" | "other";

export interface PlatformCapabilities {
  freeFilePaths: boolean;
  shareSheet: boolean;
  printing: boolean;
  systemFonts: boolean;
  ocr: boolean;
  canBeSuspended: boolean;
}

export interface RendererInfo {
  backend: string;
  available: boolean;
  detail: string;
}

export interface MemoryBudget {
  maxCacheBytes: number;
  maxPagePixels: number;
  prerenderRadius: number;
}

export interface CoreInfo {
  version: string;
  platform: PlatformKind;
  capabilities: PlatformCapabilities;
  renderer: RendererInfo;
  memoryBudget: MemoryBudget;
  features: string[];
}

export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface Point {
  x: number;
  y: number;
}

export interface Matrix {
  a: number;
  b: number;
  c: number;
  d: number;
  e: number;
  f: number;
}

export interface ObjectRef {
  number: number;
  generation: number;
}

export interface PageInfo {
  index: number;
  object: ObjectRef;
  mediaBox: Rect;
  cropBox: Rect;
  rotation: number;
  widthPt: number;
  heightPt: number;
  contentStreamCount: number;
  annotationCount: number;
}

export interface HistoryEntry {
  label: string;
  pageIndex: number | null;
}

export interface HistoryView {
  canUndo: boolean;
  canRedo: boolean;
  undo: HistoryEntry[];
  redo: HistoryEntry[];
}

export type DocumentHandle =
  | { kind: "path"; value: string }
  | { kind: "contentUri"; value: string }
  | { kind: "securityScopedBookmark"; value: string }
  | { kind: "inMemory"; value: { name: string } };

export interface DocumentSource {
  handle: DocumentHandle | null;
  displayName: string;
}

export interface DocumentSummary {
  id: number;
  source: DocumentSource;
  pdfVersion: string;
  pageCount: number;
  pages: PageInfo[];
  dirty: boolean;
  revision: number;
  wasEncrypted: boolean;
  byteSize: number;
  originalSha256: string;
  history: HistoryView;
}

export type ShowItem = { kind: "text"; value: number[] } | { kind: "adjust"; value: number };

export interface TextRun {
  pageIndex: number;
  streamIndex: number;
  operationIndex: number;
  operator: string;
  items: ShowItem[];
  fontResource: string;
  fontSize: number;
  charSpacing: number;
  wordSpacing: number;
  horizontalScale: number;
  rise: number;
  renderMode: number;
  textMatrix: Matrix;
  ctm: Matrix;
  origin: Point;
  effectiveFontSize: number;
}

export interface PageText {
  pageIndex: number;
  runs: TextRun[];
  fonts: string[];
}

export type EditCommand =
  | { type: "rotatePage"; pageIndex: number; degrees: number }
  | { type: "setDocumentInfo"; fields: Record<string, string | null> };

export type SaveMode = "incremental" | "full";

export interface ValidationReport {
  ok: boolean;
  pdfVersion: string;
  pageCount: number;
  hasTrailingEof: boolean;
  errors: string[];
  warnings: string[];
}

export interface SaveReport {
  mode: SaveMode;
  byteSize: number;
  appendedBytes: number;
  changedObjects: number;
  validation: ValidationReport;
}

export interface RenderRequest {
  pageIndex: number;
  scale: number;
  extraRotation?: number;
}

/** A page bitmap, already unpacked from the binary reply. */
export interface RenderedPage {
  pageIndex: number;
  width: number;
  height: number;
  scale: number;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

/** Stable error codes from the core. The German wording lives in the frontend. */
export type ErrorCode =
  | "io"
  | "broken_document"
  | "password_required"
  | "wrong_password"
  | "unknown_document"
  | "unknown_page"
  | "missing_object"
  | "content_stream"
  | "font"
  | "renderer_unavailable"
  | "render"
  | "save"
  | "nothing_to_undo"
  | "nothing_to_redo"
  | "not_implemented"
  | "invalid_argument";

export interface CommandError {
  code: ErrorCode | string;
  message: string;
}
