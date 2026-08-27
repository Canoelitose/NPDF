# NPDF

Ein PDF-Editor fuer Windows, macOS, Linux, iOS und Android, der vorhandenen
Inhalt wirklich aendert statt ihn zu ueberlagern. Alles laeuft lokal. Kein Konto,
keine Cloud, keine Telemetrie.

Ein Rust-Kern, eine Oberflaeche, fuenf Ziele.

## Stand

M0, Projektgeruest, ist fertig. Was heute geht:

* Ein PDF oeffnen, auch ein beschaedigtes oder verschluesseltes, mit klarer
  Meldung statt Absturz
* Seiten mit ihren echten Massen, Drehungen und Rahmen auflisten
* Text-Runs einer Seite mit Position, Schrift und Matrix auslesen
* Eine Seite drehen und Metadaten aendern, mit Rueckgaengig und Wiederholen
* Inkrementell speichern, wobei die Originalbytes unveraendert bleiben
* Seiten mit PDFium darstellen, mit begrenztem Zwischenspeicher
* Zwei Darstellungen pixelweise vergleichen

Was noch nicht geht, steht ehrlich in `docs/milestones/M0.md`.

## Loslegen

Voraussetzungen: Rust stabil, Node 22 oder neuer, die Tauri-Voraussetzungen
deiner Plattform. Unter Linux zusaetzlich:

    sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev patchelf

Dann:

    npm install
    npm run fetch:pdfium        # laedt die Renderer-Bibliothek fuer deinen Rechner
    npm run tauri dev

Testdateien und Tests:

    npm run fetch:fixtures      # oeffentliche Beispiel-PDFs, CC0
    cargo test --workspace --all-features

Die Tests laufen auch ohne die beiden Downloads. Was eine fehlende Datei
braucht, meldet dann einen uebersprungenen Lauf statt eines Fehlers.

## Aufbau

    crates/npdf-core/     Reines Rust, keine Systemabhaengigkeit, ueberall gleich
      doc/                Dokumentmodell, Seitenbaum, Verlauf
      extract/            Content Streams nachspielen, Text finden
      edit/               Befehle mit Rueckgaengig
      fonts/              Glyphenbreiten, fehlende Zeichen, Systemschriften
      render/             Seitenbilder, Zwischenspeicher, Pixelvergleich
      save/               Inkrementell, vollstaendig, Pruefung
    src-tauri/            Die Schale, enthaelt keine PDF-Logik
      src/platform/       Die einzige Stelle mit Plattformunterschieden
    src/                  Oberflaeche, TypeScript und React
      styles/tokens.css   Farben, Radien, Abstaende, Schatten, Bewegung
    scripts/              PDFium und Testdateien laden
    docs/                 Aufbau, Abhaengigkeiten, Plattformen, Meilensteine

## Weiterlesen

* `docs/architecture.md`, warum das Original nie angefasst wird
* `docs/dependencies.md`, jede Abhaengigkeit mit Lizenz und Mobiltauglichkeit
* `docs/platforms.md`, Konten, Zertifikate und was pro Plattform anders ist
* `docs/milestones/M0.md`, was fertig ist und was offen bleibt

## Lizenz

MIT oder Apache-2.0, nach Wahl. Siehe `LICENSE-MIT` und `LICENSE-APACHE`.
