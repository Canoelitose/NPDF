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

**macOS.** Ein Mac, der beim Bauen in den Leerlauf geht, hoert auf, mit GitHub
zu reden. Nach zehn Minuten Funkstille bricht GitHub den Auftrag ab. Der Bau
laeuft oertlich weiter und erfaehrt davon erst, wenn die Maschine
zurueckkommt. Der Workflow haelt den Mac deshalb fuer die Dauer des Auftrags
wach, mit `caffeinate -dims -w $PPID`. Das haengt am Runner-Prozess selbst und
endet mit dem Auftrag, egal wie er endet.

**Windows.** Lange Pfade einschalten, sonst laufen Rust und `node_modules`
gegen die alte Grenze von 260 Zeichen. Dazu eine Defender-Ausnahme fuer das
Arbeitsverzeichnis: Rust schreibt zehntausende kleiner Dateien, und jede
einzeln zu pruefen kostet ein Vielfaches der Bauzeit. Beides macht
`setup-windows.ps1` mit.

## Pruefen, ob es wirklich laeuft

Die Variable allein beweist nichts. Erst wenn ein Lauf auf dem eigenen Runner
durchgelaufen ist und seine Installationsdatei als Artefakt haengt, ist die
Sache fertig. Unter Actions steht am Auftrag, welche Maschine ihn genommen hat.

## Der Mac darf nicht in den Leerlauf gehen

Das ist kein Schoenheitsfehler, es kostet ganze Laeufe. Zwei Auspraegungen:

* **Zwischen Jobs.** Schlaeft der Mac im Leerlauf ein, meldet sich der Runner ab
  und wartende Jobs bleiben in der Warteschlange stehen, ohne Fehler, ohne Ende.
* **Waehrend eines Jobs.** Die Verbindung reisst ab, GitHub sieht zehn Minuten
  lang nichts mehr und bricht den Auftrag ab. Woran man es erkennt: der Auftrag
  ist nach exakt zehn Minuten zu Ende, das Protokoll bricht mitten im
  Uebersetzen ab und endet mit `The operation was canceled`, ohne dass davor ein
  Fehler steht.

Zwei Massnahmen, beide noetig:

    sudo pmset -a sleep 0 disksleep 0

erledigt der Einrichtungsschritt in `scripts/runner/setup-macos.sh` mit. Und im
Workflow steht der Wachhalter als **erster** Schritt des Jobs. Weiter unten
nuetzt es nichts, dann kann der Rechner vorher wegdriften.

Auf den Aufruf kommt es dabei an:

    nohup caffeinate -dims -w $PPID >/dev/null 2>&1 &

Ohne `-u`. Das Kennzeichen erklaert den Benutzer fuer aktiv, aber ohne `-t` nur
fuenf Sekunden lang, danach beendet sich `caffeinate` und der Schutz ist weg.
Zu erkennen war das daran, dass der Schritt genau fuenf Sekunden brauchte statt
sofort fertig zu sein. Die Umleitung sorgt dafuer, dass der Schritt nicht auf
das Schliessen der Ausgabekanaele wartet.

Wenn du den Ruhezustand wieder willst:

    sudo pmset -a sleep 1
