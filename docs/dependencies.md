# Abhaengigkeiten und Lizenzen

Stand M0. Jede Zeile wurde geprueft: Lizenz, Reinheit in Rust und ob sie fuer
iOS und Android uebersetzt werden kann.

Das Projekt selbst steht unter MIT oder Apache-2.0, nach Wahl. Alles unten ist
damit vertraeglich. **MuPDF wird nicht verwendet**, weil es unter AGPL steht und
das die App unter Copyleft zwingen wuerde. Aus demselben Grund kommt kein
Poppler und kein qpdf mit GPL-Bindung in Frage.

## Rust, Kern

| Crate | Version | Lizenz | Reines Rust | iOS | Android | Wofuer |
|---|---|---|---|---|---|---|
| `lopdf` | 0.44 | MIT | ja | ja | ja | Objekte, Content Streams, inkrementelles Speichern |
| `ttf-parser` | 0.25 | MIT oder Apache-2.0 | ja | ja | ja | Glyphenbreiten, Zeichentabellen, Metriken |
| `serde`, `serde_json` | 1 | MIT oder Apache-2.0 | ja | ja | ja | Bruecke zur Oberflaeche |
| `thiserror` | 2 | MIT oder Apache-2.0 | ja | ja | ja | Fehlertypen |
| `parking_lot` | 0.12 | MIT oder Apache-2.0 | ja | ja | ja | Sperren |
| `sha2` | 0.10 | MIT oder Apache-2.0 | ja | ja | ja | Pruefsumme der geoeffneten Datei |
| `log` | 0.4 | MIT oder Apache-2.0 | ja | ja | ja | Protokoll |

## Rust, noch nicht eingebunden, ab M4 vorgesehen

| Crate | Version | Lizenz | Reines Rust | Wofuer |
|---|---|---|---|---|
| `rustybuzz` | 0.20 | MIT | ja | Shaping, Ligaturen, komplexe Schriftsysteme |
| `subsetter` | 0.2 | MIT oder Apache-2.0 | ja | Teilmengen einbetten |
| `flate2` | 1 | MIT oder Apache-2.0 | ja, mit `rust_backend` | Streams packen und entpacken |
| `image` | 0.25 | MIT oder Apache-2.0 | ja | Bilder ersetzen und ausgeben, ab M5 |

Diese vier sind bewusst noch nicht als Abhaengigkeit eingetragen. Sie stehen
hier, damit die Lizenzpruefung schon erledigt ist, wenn sie gebraucht werden.

## Rendern

| Baustein | Lizenz | Wofuer |
|---|---|---|
| PDFium | BSD-3-Clause | Die eigentliche Seitendarstellung |
| `pdfium-render` 0.9 | MIT oder Apache-2.0 | Rust-Anbindung an PDFium |
| Vorgebaute Binaerpakete, bblanchon/pdfium-binaries | Apache-2.0 fuer die Verpackung, BSD-3-Clause fuer PDFium | Fertige Bibliotheken pro Ziel |

PDFium ist C++ und laesst sich nicht mit cargo bauen. Die vorgebauten Pakete
decken alle fuenf Ziele ab. Die Version ist auf `chromium/7881` festgelegt, weil
`pdfium-render` gegen genau diese Schnittstelle gebaut ist. Wer eine der beiden
Zahlen aendert, muss die andere mitaendern, sonst laedt die Bibliothek und
stuerzt beim ersten Aufruf ab.

## Schale und Oberflaeche

| Baustein | Version | Lizenz |
|---|---|---|
| Tauri | 2 | MIT oder Apache-2.0 |
| React, React DOM | 19 | MIT |
| Vite | 7 | MIT |
| TypeScript | 5.7 | Apache-2.0 |

## Warum React und nicht Svelte

Beides waere tragbar. Ausschlaggebend war die Textebene. Ab M3 liegt ueber dem
gerenderten Bild eine unsichtbare Schicht, die auf den echten Glyphenpositionen
sitzt, Klicks in Zeichenpositionen umrechnet, Auswahlgriffe zeichnet und mit dem
Eingabesystem des Betriebssystems zusammenarbeiten muss, auch mit der Tastatur
auf dem Telefon. Das ist eine Menge sehr genauer, direkter DOM-Arbeit, und dafuer
gibt es im React-Umfeld die reifere Sammlung an fertigen Bausteinen, von
Ziehen und Ablegen fuer die Miniaturansicht bis zur Virtualisierung langer
Seitenlisten. Dazu kommt, dass Tauri-Beispiele und Fehlermeldungen fast immer
React voraussetzen, was bei einem Projekt dieser Groesse Zeit spart.

## Bewusst nicht verwendet

| Baustein | Grund |
|---|---|
| MuPDF | AGPL |
| Poppler | GPL |
| qpdf | Apache-2.0, aber C++ und ueberschneidet sich mit lopdf |
| pdf.js | Waere ein zweiter Renderer im Web-Layer, doppelte Wahrheit ueber die Seite |
| Tesseract | Erst ab M8, und auf Mobilgeraeten wahrscheinlich gar nicht, siehe docs/platforms.md |
