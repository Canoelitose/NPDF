# NPDF auf eigenen Maschinen bauen

GitHub rechnet nur seine **eigenen** Maschinen gegen das Monatskontingent.
Meldest du einen eigenen Rechner als Runner an, kostet Bauen nichts mehr, und
es geht meist schneller, weil die Abhaengigkeiten zwischen zwei Bauten liegen
bleiben.

Sonst aendert sich nichts: dieselben Ausloeser, dieselbe Matrix, dieselben
Installationsdateien.

Das Verfahren ist dasselbe wie im Schwesterprojekt Organ. Drei
Repository-Variablen entscheiden pro System, welche Maschine baut. Ist eine
Variable leer, laeuft der Auftrag wie bisher auf GitHubs Maschine. Am Workflow
ist zum Umschalten nichts zu aendern.

## Umschalten

Settings, Secrets and variables, Actions, Variables, New repository variable.

| Name | Wert | betrifft |
|---|---|---|
| `LINUX_RUNNER` | `npdf-linux` | Kern, Frontend, Android, Linux-Installer |
| `MACOS_RUNNER` | `npdf-mac` | DMG, App-Buendel, iOS |
| `WINDOWS_RUNNER` | `npdf-win` | MSI und NSIS |

Der Wert ist das **Etikett** des Runners, nicht sein Name. Die Skripte unter
`scripts/runner/` vergeben es beim Anmelden. Zurueck zu GitHubs Maschinen geht
es, indem du die Variable loeschst.

Ein Auftrag an einen abgeschalteten Runner **wartet**, er scheitert nicht, bis
zu einen Tag lang. Ist eine Maschine laenger weg, loesche ihre Variable vorher.

## Anmelden

Das Registrierungs-Token steht unter Settings, Actions, Runners, New
self-hosted runner. Es gilt eine Stunde und gehoert in keine Datei.

    Linux:   sudo bash scripts/runner/setup-linux.sh <TOKEN>
    macOS:   bash scripts/runner/setup-macos.sh <TOKEN>
    Windows: powershell -ExecutionPolicy Bypass -File scripts\runner\setup-windows.ps1 -Token <TOKEN>

Die Skripte richten jeweils ein eigenes Verzeichnis ein, `actions-runner-npdf`
beziehungsweise `C:\a-npdf`. Damit kann dieselbe Maschine daneben einen Runner
fuer ein anderes Repository betreiben, ohne dass sich die beiden ins Gehege
kommen.

## Vor dem ersten Lauf, Pflicht

Das Repository ist oeffentlich. Ein selbstgehosteter Runner fuehrt aus, was ein
Workflow ihm sagt, auch aus dem Pull Request eines Fremden. Unter Settings,
Actions, General muss **Fork pull request workflows from outside collaborators**
auf "Require approval for all external contributors" stehen, bevor der erste
Runner laeuft.

Organ hat dieses Problem nicht, weil es privat ist. Hier ist der Schalter die
einzige Absicherung.

## Was der Bau selbst mitbringt

| | wer es besorgt |
|---|---|
| Node 22 | der Bau (`actions/setup-node`) |
| Rust stabil | der Bau (`dtolnay/rust-toolchain`) |
| PDFium | der Bau (`scripts/fetch-pdfium.mjs`) |
| NSIS und WiX | Tauri laedt sie beim ersten Windows-Bau nach |

Alles andere muss auf der Maschine liegen. Die Skripte erledigen es.

## Was pro System leicht uebersehen wird

**Linux.** Der Runner-Benutzer hat bewusst kein sudo, also kann der Bau seine
Pakete nicht selbst holen. Der apt-Schritt im Workflow ueberspringt sich
deshalb auf eigenen Maschinen, erkennbar an
`runner.environment == 'github-hosted'`. `libfuse2` ist das Paket, das am
teuersten fehlt: `linuxdeploy` ist selbst ein AppImage und braucht FUSE, das
AppImage scheitert also als letztes Buendel nach dem ganzen Uebersetzen. Heisst
es auf deiner Ausgabe nicht so, ist es `libfuse2t64`.

**macOS.** Ein Mac, der beim Bauen einschlaeft, nimmt den Runner mit. Der
Auftrag scheitert dann ohne Protokoll, weil niemand mehr da ist, der eines
hochladen koennte. Der Workflow haelt den Mac deshalb fuer die Dauer des
Auftrags wach, mit `caffeinate -dimsu -w $PPID`. Das haengt am Runner-Prozess
selbst und endet mit dem Auftrag, egal wie er endet.

**Windows.** Lange Pfade einschalten, sonst laufen Rust und `node_modules`
gegen die alte Grenze von 260 Zeichen. Dazu eine Defender-Ausnahme fuer das
Arbeitsverzeichnis: Rust schreibt zehntausende kleiner Dateien, und jede
einzeln zu pruefen kostet ein Vielfaches der Bauzeit. Beides macht
`setup-windows.ps1` mit.

## Pruefen, ob es wirklich laeuft

Die Variable allein beweist nichts. Erst wenn ein Lauf auf dem eigenen Runner
durchgelaufen ist und seine Installationsdatei als Artefakt haengt, ist die
Sache fertig. Unter Actions steht am Auftrag, welche Maschine ihn genommen hat.
