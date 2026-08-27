# Prompt: NPDF auf einer Maschine einrichten

Diesen Text in Claude Code auf der jeweiligen Maschine einfuegen. Er funktioniert
unveraendert unter Windows, macOS und Linux, der Agent erkennt selbst, wo er ist.

Vorher zwei Dinge bereitlegen:

1. Falls du den Runner anmelden willst, ein Registrierungs-Token von
   `https://github.com/Canoelitose/NPDF/settings/actions/runners/new`.
   Es ist eine Stunde gueltig.
2. Falls du nur bauen willst, brauchst du gar nichts.

---

```text
Du richtest auf DIESER Maschine das Projekt NPDF ein, einen PDF-Editor auf
Basis von Tauri v2, Rust und React. Arbeite selbstaendig und melde am Ende,
was geht und was nicht.

Repository: https://github.com/Canoelitose/NPDF
Zweig: claude/pdf-editor-tauri-app-h1kuop

Sprache fuer alles, was ich lesen soll: Deutsch, ohne scharfes S und ohne
Gedankenstriche. Code und Kommentare auf Englisch.

TEIL A, PFLICHT: die App lokal zum Laufen bringen

1. Stelle fest, auf welchem Betriebssystem und welcher Architektur du bist,
   und sage es mir.
2. Klone das Repository, falls es noch nicht da ist, und wechsle auf den Zweig
   oben.
3. Installiere die Voraussetzungen, falls sie fehlen: Rust stabil, Node 22 oder
   neuer, und die Tauri-Voraussetzungen deiner Plattform. Unter Linux sind das
   libwebkit2gtk-4.1-dev, libgtk-3-dev, librsvg2-dev, patchelf,
   libappindicator3-dev und xdg-utils. Unter Windows die Visual Studio Build
   Tools mit C++ und die WebView2-Laufzeit. Unter macOS die Xcode
   Kommandozeilenwerkzeuge.
4. npm install
5. npm run fetch:pdfium
   Das laedt die Renderer-Bibliothek fuer deine Plattform. Ohne sie startet die
   App zwar, zeigt aber keine Seiten an.
6. Pruefe, dass alles gruen ist:
   cargo test --workspace --all-features
   npm run typecheck
   cargo clippy --all-targets --all-features -- -D warnings
   Bei den Tests hilft es, NPDF_PDFIUM_PATH auf das Verzeichnis mit der
   Bibliothek zu setzen, sonst ueberspringen die Renderer-Tests sich selbst.
7. Baue die Installationsdateien fuer diese Plattform:
   npm run tauri -- build
   Unter macOS entsteht dabei eine DMG, unter Windows MSI und NSIS, unter Linux
   AppImage und DEB. Sag mir, wo die Dateien liegen und wie gross sie sind.
8. Starte die App einmal wirklich und sieh sie dir an. Pruefe:
   erscheint das Fenster, ist der Hintergrund violett und nicht rot, laesst
   sich mit Strg oder Cmd und O ein PDF oeffnen, wird die Seite dargestellt.
   Wenn du eine Aufnahme des Bildschirms machen kannst, mach eine.
9. Melde jeden Fehler mit der genauen Meldung. Rate nicht, wenn du etwas nicht
   pruefen konntest, sag dass du es nicht geprueft hast.

TEIL B, OPTIONAL: diese Maschine als GitHub-Actions-Runner anmelden

Nur ausfuehren, wenn ich dir ein Registrierungs-Token gegeben habe.

WICHTIG, vorher lesen: das Repository ist oeffentlich. Ein selbstgehosteter
Runner fuehrt aus, was ein Workflow ihm sagt, auch aus dem Pull Request eines
Fremden. Bevor der Runner laeuft, muss unter
Settings, Actions, General, Fork pull request workflows from outside
collaborators die Einstellung auf "Require approval for all external
contributors" stehen. Erinnere mich daran und frage nach, ob das erledigt ist,
bevor du den Runner startest.

Im Repository liegen fertige Skripte, nimm das passende:
  Linux:   sudo bash scripts/runner/setup-linux.sh <TOKEN>
  Windows: powershell -ExecutionPolicy Bypass -File scripts\runner\setup-windows.ps1 -Token <TOKEN>
  macOS:   bash scripts/runner/setup-macos.sh <TOKEN>

Danach pruefen, dass der Runner unter
https://github.com/Canoelitose/NPDF/settings/actions/runners als Idle steht,
und mir seinen Namen und seine Bezeichnungen nennen.

Aendere die Workflows NICHT selbst auf den Runner um. Sag mir nur Bescheid,
dass er bereit ist.

GRENZEN, halte dich daran

- Aendere nichts am Quelltext, ausser ich sage es dir. Dies ist eine
  Einrichtung, keine Weiterentwicklung.
- Wenn ein Bau scheitert, hole die genaue Fehlermeldung und zeige sie mir,
  statt drumherum zu bauen.
- Committe und pushe nichts ohne meine Zustimmung.
```
