# Plattformen, Konten und Zertifikate

## Was du brauchst

Du hast angegeben, dass du noch keine Konten hast. Die CI baut deshalb alles
unsigniert, und die Signierschritte sind bereits vorhanden, aber abgeschaltet.
Eingeschaltet werden sie ueber die Repository-Variable `ENABLE_SIGNING`, sobald
die Geheimnisse hinterlegt sind.

| Ziel | Was noetig ist | Kosten | Ohne das |
|---|---|---|---|
| Windows | Zertifikat einer Zertifizierungsstelle, EV oder OV | etwa 200 bis 400 Euro im Jahr | SmartScreen warnt beim ersten Start |
| macOS | Apple Developer Program, Developer ID | 99 US-Dollar im Jahr | Gatekeeper verweigert den Start, Umweg ueber das Kontextmenue noetig |
| Linux | nichts | keine | nichts |
| iOS | Apple Developer Program, Zertifikat, Bereitstellungsprofil | im selben Programm enthalten | Kein Bau auf ein Geraet, nur der Simulator |
| Android | Keystore, selbst erzeugt. Fuer Google Play zusaetzlich ein Entwicklerkonto | Keystore kostenlos, Play einmalig 25 US-Dollar | Debug-APK laeuft, Play nimmt nichts an |

Der guenstigste sinnvolle Einstieg ist der Android-Keystore, weil er nichts
kostet und sofort geht:

    keytool -genkey -v -keystore npdf.jks -keyalg RSA -keysize 4096 \
      -validity 10000 -alias npdf

Die Datei gehoert nicht ins Repository. In der CI liegt sie als
base64-kodiertes Geheimnis.

## Geheimnisse, die die CI erwartet

| Name | Wofuer |
|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` | Developer-ID-Zertifikat fuer macOS |
| `APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID` | Beglaubigung bei Apple |
| `APPLE_ID`, `APPLE_PASSWORD` | App-spezifisches Kennwort fuer die Beglaubigung |
| `IOS_CERTIFICATE`, `IOS_CERTIFICATE_PASSWORD`, `IOS_MOBILE_PROVISION` | iOS |
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | Windows |
| `ANDROID_KEYSTORE`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` | Android |

## Entwickeln unter Windows

Dein Rechner deckt Windows, Android und die Weboberflaeche vollstaendig ab.
Fuer macOS und iOS brauchst du einen Mac, deshalb baut die CI diese beiden.
Was du auf Windows brauchst:

* Rust, stabile Ausgabe, mit `rustup`
* Node 22 oder neuer
* Visual Studio Build Tools mit dem C++-Arbeitsbereich
* WebView2, auf Windows 11 bereits vorhanden
* Fuer Android: Android Studio, SDK, NDK und ein JDK 17

## Was auf welcher Plattform anders ist

### Dateizugriff

Weder iOS noch Android geben einer App freie Dateipfade.

* **Android** nutzt das Storage Access Framework. Die Auswahl liefert eine
  `content://`-Adresse, keinen Pfad. Nur die Systemschnittstelle kann sie
  aufloesen.
* **iOS** arbeitet mit der Dateien-App und mit sicherheitsbezogenen Lesezeichen.
  Ein Griff auf eine Datei ausserhalb des eigenen Bereichs muss explizit
  geoeffnet und wieder geschlossen werden, sonst laeuft das Recht ab.

Deshalb gibt es die zwei Wege in den Kern, siehe docs/architecture.md. Der
Dokument-Lebenszyklus ist von Anfang an so gebaut, dass ein PDF auch aus einer
anderen App oder aus einem geteilten Inhalt kommen kann.

Die Datei-Zuordnung fuer PDF steht bereits in `src-tauri/tauri.conf.json`. Auf
Android und iOS wird sie beim Erzeugen des jeweiligen Projekts uebernommen, das
Entgegennehmen eines geteilten Inhalts kommt in M7.

### Arbeitsspeicher

Auf Mobilgeraeten gilt ein deutlich engeres Mass, siehe `render::MemoryBudget`.
Geht die App in den Hintergrund, gibt sie Speicher zurueck. Ohne das beendet das
System die App im Hintergrund, und der Nutzer verliert seine Arbeit.

### PDFium

Das war das groesste Risiko in M0 und ist ausgeraeumt. Die vorgebauten
Bibliotheken decken alle fuenf Ziele ab:

| Ziel | Ablage | Bindung |
|---|---|---|
| Windows | `pdfium.dll` neben der Anwendung | zur Laufzeit geladen |
| macOS | `libpdfium.dylib` im Paket | zur Laufzeit geladen |
| Linux | `libpdfium.so` neben der Anwendung | zur Laufzeit geladen |
| Android | `libpdfium.so` in `jniLibs/<abi>` | zur Laufzeit geladen |
| iOS | `libpdfium.a` | fest ins Programm gebunden |

iOS erlaubt keine dynamische Bibliothek ausserhalb eines Frameworks, deshalb
wird dort statisch gebunden. Das geschieht automatisch, sobald fuer iOS gebaut
wird, siehe die Zieldefinition in `crates/npdf-core/Cargo.toml`. Der Bau
erwartet nur, dass `PDFIUM_STATIC_LIB_PATH` auf das Verzeichnis mit der
`libpdfium.a` zeigt.

Falls sich das auf einem Ziel doch als untragbar erweist, ist der Ausweg
vorbereitet: `render::PageRenderer` ist eine Schnittstelle, hinter die ein
reiner Rust-Renderer nur fuer dieses Ziel treten kann. Nichts ausserhalb des
Moduls `render` weiss, wer die Pixel malt.

### OCR

Tesseract ist C++ und auf Mobilgeraeten aufwendig zu bauen. Der Plan bleibt: auf
dem Schreibtisch als abschaltbare Zusatzfunktion in M8, auf iOS und Android
vorerst gar nicht, und in der Oberflaeche dann ausgeblendet statt halb da.
`PlatformCapabilities::ocr` sagt das dem Frontend bereits heute.
