/** German wording for numbers and error codes. */

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["kB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toLocaleString("de-DE", { maximumFractionDigits: 1 })} ${units[unit]}`;
}

export function formatPages(count: number): string {
  return count === 1 ? "1 Seite" : `${count} Seiten`;
}

const messages: Record<string, string> = {
  io: "Die Datei konnte nicht gelesen oder geschrieben werden.",
  broken_document: "Diese Datei ist kein PDF oder ihre Struktur ist beschaedigt.",
  password_required: "Dieses PDF ist verschluesselt und braucht ein Kennwort.",
  wrong_password: "Das Kennwort wurde nicht angenommen.",
  unknown_document: "Dieses Dokument ist nicht mehr geoeffnet.",
  unknown_page: "Diese Seite gibt es nicht.",
  missing_object: "Ein Objekt fehlt in der Datei.",
  content_stream: "Der Inhalt dieser Seite konnte nicht gelesen werden.",
  font: "Mit der Schrift stimmt etwas nicht.",
  renderer_unavailable: "Die Seitendarstellung steht nicht bereit.",
  render: "Die Seite konnte nicht dargestellt werden.",
  save: "Das Speichern ist fehlgeschlagen.",
  nothing_to_undo: "Es gibt nichts zum Rueckgaengigmachen.",
  nothing_to_redo: "Es gibt nichts zum Wiederherstellen.",
  not_implemented: "Diese Funktion gibt es noch nicht.",
  invalid_argument: "Diese Eingabe passt nicht.",
};

export function germanMessage(code: string, fallback: string): string {
  return messages[code] ?? fallback;
}
