# PDFium

Diese Verzeichnisse nehmen die PDFium-Bibliothek auf. Sie liegen absichtlich leer
im Repository, damit der Paketierer sie immer findet.

Gefuellt werden sie mit:

    npm run fetch:pdfium

Das Skript laedt die vorgebauten Bibliotheken von bblanchon/pdfium-binaries,
prueft die Pruefsumme und legt sie hier ab. PDFium selbst steht unter der
BSD-3-Clause-Lizenz, die Vorlage der Binaerpakete unter Apache-2.0. Die Dateien
gehoeren nicht in die Versionsverwaltung, siehe .gitignore.

Ablage pro Ziel:

| Ziel                | Datei                          |
|---------------------|--------------------------------|
| Windows             | lib/pdfium.dll                 |
| macOS               | lib/libpdfium.dylib            |
| Linux               | lib/libpdfium.so               |
| Android             | jniLibs/<abi>/libpdfium.so     |
| iOS                 | lib/libpdfium.a, statisch gelinkt |
