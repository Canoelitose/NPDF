# Aufbau

## Grundgedanke

Die Datei, die du oeffnest, wird nie veraendert. Sie liegt als Bytefolge im
Speicher und bleibt genau so. Jede Aenderung landet in einer zweiten,
anfangs leeren Ebene, die nur die Objekte enthaelt, die ein Befehl angefasst
hat. Beim Speichern werden die Originalbytes unveraendert geschrieben und die
zweite Ebene als inkrementelle Aktualisierung angehaengt.

Das ist keine Sparmassnahme, sondern die Antwort auf die schwierigste
Anforderung des Projekts. Lesezeichen, Metadaten, Formularfelder, eingebettete
Dateien, digitale Zusaetze und alles andere, was wir nicht verstehen, ueberleben
dadurch von selbst und nicht aus Sorgfalt. Der Test dazu ist hart: nach dem
Speichern muessen die ersten `n` Bytes der Ausgabe Byte fuer Byte der Eingabe
entsprechen, sonst wird das Speichern abgebrochen.

## Ebenen

    Oberflaeche, TypeScript und React
        |  Tauri-Commands, getippt in src/lib/ipc.ts
        v
    Schale, src-tauri, Rust
        |  reicht durch, enthaelt keine PDF-Logik
        v
    Kern, crates/npdf-core, reines Rust
        |  PlatformServices, das einzige Loch nach aussen
        v
    Betriebssystem

Die Schale ist absichtlich duenn. Alles, was ein Befehl tut, steht im Kern, und
der Kern laesst sich ohne Fenster testen. Der ganze Ablauf vom Oeffnen ueber das
Aendern bis zum Speichern hat Tests, die auf jedem der fuenf Ziele laufen.

## Module des Kerns

| Modul | Aufgabe | Stand nach M0 |
|---|---|---|
| `doc` | Dokumentmodell, Seitenbaum, Undo-Stack | steht |
| `extract` | Content Streams nachspielen, Text-Runs finden | Grundlage steht, Zeilenbildung folgt in M2 |
| `edit` | Befehle mit Rueckgaengig und Wiederholen | Mechanik steht, zwei Befehle als Nachweis |
| `fonts` | Glyphenbreiten, fehlende Zeichen, Systemschriften finden | Schriftseite steht, PDF-Seite folgt in M2 und M3 |
| `render` | Seitenbilder, Zwischenspeicher, Pixelvergleich | steht |
| `save` | inkrementell, vollstaendig, Pruefung | steht |
| `session` | Alle offenen Dokumente einer laufenden App | steht |
| `platform` | Das einzige Loch nach aussen | steht |

## Rueckgaengig

Ein Befehl merkt sich vor der Ausfuehrung, wie jedes Objekt aussah, das er
anfassen wird, und schreibt erst danach. Rueckgaengig stellt den gemerkten
Zustand wieder her, Wiederholen fuehrt den Befehl noch einmal aus. Der
Mechanismus muss nicht wissen, was ein Befehl tut. Jeder neue Befehl bekommt
Rueckgaengig geschenkt, solange er jedes Objekt, das er aendert, vorher anmeldet.

Scheitert ein Befehl auf halbem Weg, wird der gemerkte Zustand sofort
zurueckgespielt. Es gibt keinen halb ausgefuehrten Befehl im Verlauf.

## Plattformunterschiede

`npdf_core::platform::PlatformServices` ist die einzige Stelle, an der
Plattformunterschiede erlaubt sind. Ausserhalb von `src-tauri/src/platform/`
steht in der gesamten Schale kein `cfg(target_os)`.

Der wichtigste Unterschied betrifft den Dateizugriff. Auf dem Schreibtisch gibt
es echte Pfade und der Kern liest und schreibt selbst. Auf iOS und Android gibt
es keine freien Pfade. Ein Dokument kommt als Griff aus der Dokumentenauswahl,
aus einem geteilten Inhalt oder aus einer anderen App, und nur die Schale kann
daraus Bytes machen. Deshalb gibt es zwei Wege in den Kern hinein und zwei
heraus:

| Weg | Schreibtisch | Mobil |
|---|---|---|
| Oeffnen | `open_document_path` | `open_document_bytes` |
| Speichern | `save_document` | `save_document_bytes`, die Schale schreibt |

## Speicher

Seitenbilder sind das einzige, was wirklich Speicher kostet. Der Zwischenspeicher
ist nach Bytes begrenzt, nicht nach Anzahl, weil eine Plakatseite so viel wiegt
wie fuenfzig Textseiten. Die Grenzen stehen an einer Stelle, in
`render::MemoryBudget`, und sind auf Mobilgeraeten deutlich enger. Geht die App
in den Hintergrund, ruft die Oberflaeche `release_memory` auf, der
Zwischenspeicher schrumpft auf ein Achtel und die Renderer werden weggeworfen.

## Die Bruecke

Kleine Antworten gehen als JSON. Seitenbilder und gespeicherte Dokumente nicht:
eine A4-Seite bei zweihundert Prozent sind rund acht Megabyte, und als
JSON-Array aus Zahlen waere das unbrauchbar langsam. Beide gehen deshalb als
Binaerblock mit einem festen Kopf, der in `src-tauri/src/commands/dto.rs`
dokumentiert und getestet ist.

## Was noch nicht steht

* Die Textmatrix wandert nach einem Zeichenbefehl nur um die ausdruecklichen
  Verschiebungen aus `TJ` weiter. Der Glyphenvorschub braucht die Breiten der
  eingebetteten Schrift und kommt in M2. Bis dahin wird ein zweiter Lauf auf
  derselben Zeile ohne eigenes `Td` am Zeilenanfang gemeldet.
* Formular-XObjects werden erkannt, aber nicht betreten. Text in einem
  eingebetteten Formular fehlt in der Ausgabe, ebenfalls M2.
* Zeichencodes werden noch nicht ueber die Schriftkodierung gelesen.
  `text_lossy` ist eine Notloesung fuer die Fehlersuche, keine Umwandlung.
