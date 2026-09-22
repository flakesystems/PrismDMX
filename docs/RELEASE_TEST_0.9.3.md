# Abnahmetest am Test-Rig — Build 0.9.3 (S59 + S61 + S62 + S30 + S30b)

**Gilt für:** den Build von `master` nach dem Merge von **S30** — enthält **S59** (das
Controls-Menü, die Tastenwörter, die Lampen, das Jogwheel), **S61** (die
Fixture-Bibliothek ist GDTF), **S62** (wie eine Bibliothek hereinkommt: MVR,
`.gdtf`-Import, der Login zu GDTF Share) und **S30** mit **S30b** (der 3D-Viewer,
als vollständiger Visualizer neu gebaut). Die Version im Installer ist weiterhin **`0.9.2`**:
0.9.3 ist noch nicht getaggt, und was hineinkommt, steht in `CHANGELOG.md` unter
*Noch nicht veröffentlicht*.

**Rig:** der eigene Test-Rig, Show `default.prism`, `machine.json` wie auf dem
Entwicklungsrechner.

**Die vorige Runde** steht in `docs/RELEASE_TEST.md` und ist abgehakt und
abgenommen (2026-09-20). Diese Datei ist die **neue** Runde und wiederholt sie
nicht; nur Kapitel 4 geht kurz über das, was vorher schon ging.

Jeder Punkt sagt **wo**, **was zu tun ist** und **was passieren muss**. Ein
Kästchen, das nicht passiert, ist ein Befund: bitte mit Nummer (z. B. *T-GDTF.4*)
als GitHub-Issue oder in `docs/ISSUES.md` melden.

---

## 0. Bauen und installieren

Der Befehl steht in **`docs/BUILD_INSTALLER.md`** — dort ausführlich, mit den
Fallen. Kurzfassung, in einer PowerShell im Projektverzeichnis:

```powershell
cargo test -p prism-domain                 # erzeugt ui\src\bindings\
cd ui; npm ci; cd ..
cargo build --release -p prismd
cd crates\prism-app
..\..\ui\node_modules\.bin\tauri build --config tauri.bundle.conf.json
cd ..\..
```

Das Ergebnis:

```
target\release\bundle\nsis\PrismDMX_0.9.2_x64-setup.exe
```

> **Vor dem Bauen die Bibliothek installieren, sonst liefert die `.exe` keine
> mit.** Der Installer packt `profiles\fixtures\` so ein, wie es beim Bauen
> aussieht. Liegt dort nur `SOURCE.md`, bekommt das installierte Pult **vier
> eingebaute Profile und sonst nichts**. Siehe `docs/BUILD_INSTALLER.md` §3 —
> für diesen Test reicht auch der Weg über den eigenen Ordner (T-GDTF.1), dann
> ist eine leere Bibliothek in Ordnung.

- [x] **T-0.1** Installer auf dem Rig ausführen. Eine installierte 0.9.2 wird
      dabei **überschrieben** — gleiche Versionsnummer.
- [x] **T-0.2** Vor dem ersten Start nach `%APPDATA%\PrismDMX\` kopieren:
      `default.prism`, `machine.json` **und den Ordner `fixtures\`**.
- [ ] **T-0.3** PrismDMX starten. Kopfzeile: grüner Punkt **Connected**.
      X-Touch-Scribble-Strips leuchten.
- [ ] **T-0.4** *Settings* → *Outputs*: Art-Net liest **OK**, sobald der Node
      antwortet, Open DMX liest **OK**.

> **Achtung, S59 ersetzt Ihre Tastenbelegung.** Die mitgelieferte Tabelle ist
> jetzt `profiles\surface\xtouch.json`, und der Daemon **übergeht eine in
> `machine.json` gespeicherte Tabelle einer älteren Generation** und sagt es im
> Log. Das ist Absicht (S59). Ihre alte Tabelle bleibt in `machine.json` stehen
> und lässt sich weiter exportieren — aber was das Pult tut, kommt ab jetzt aus
> der neuen. Wenn Sie eigene Bindungen hatten: vorher in *Controls* →
> **Export** sichern.

---

## 1. Neu — die Fixture-Bibliothek ist GDTF (S61)

### Sich eine Testdatei bauen, ohne irgendetwas herunterzuladen

GDTF Share hat **keinen anonymen Download**. Für diesen Test brauchen Sie weder
ein Konto noch eine echte Herstellerdatei: eine `.gdtf` ist ein ZIP mit einer
`description.xml` darin, und PowerShell kann das von Haus aus. **Dieser Weg ist
gegen den echten Reader geprüft** — die erwarteten Werte unten sind gemessen und
nicht geschätzt.

In einer PowerShell:

```powershell
$dir = "$env:APPDATA\PrismDMX\fixtures"
New-Item -ItemType Directory -Path $dir -Force | Out-Null
$tmp = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "gdtf-test") -Force

@'
<?xml version="1.0" encoding="UTF-8"?>
<GDTF DataVersion="1.2">
  <FixtureType Name="Rig Test Light" ShortName="RTL" Manufacturer="PrismDMX Test"
               FixtureTypeID="9F4A0000-0000-4000-8000-00000000R161">
    <AttributeDefinitions><Attributes>
      <Attribute Name="Dimmer" Pretty="Dim"/><Attribute Name="Gobo1" Pretty="G1"/>
    </Attributes></AttributeDefinitions>
    <Wheels><Wheel Name="Gobo1">
      <Slot Name="Open"/>
      <Slot Name="Triangles" MediaFileName="gobo_triangles"/>
      <Slot Name="Dots" MediaFileName="gobo_dots"/>
    </Wheel></Wheels>
    <Models><Model Name="Body" File="body" Length="0.34" Width="0.34" Height="0.55"/></Models>
    <Geometries><Geometry Name="Body" Model="Body" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">
      <Beam Name="Beam" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,400,1}"
            BeamAngle="13" LuminousFlux="11000" ColorTemperature="6500"/>
    </Geometry></Geometries>
    <DMXModes><DMXMode Name="Mode 1" Geometry="Body"><DMXChannels>
      <DMXChannel Offset="1,2"><LogicalChannel Attribute="Pan">
        <ChannelFunction Attribute="Pan" PhysicalFrom="-270" PhysicalTo="270"/></LogicalChannel></DMXChannel>
      <DMXChannel Offset="3"><LogicalChannel Attribute="Tilt">
        <ChannelFunction Attribute="Tilt" PhysicalFrom="-135" PhysicalTo="135"/></LogicalChannel></DMXChannel>
      <DMXChannel Offset="4"><LogicalChannel Attribute="Dimmer">
        <ChannelFunction Attribute="Dimmer"/></LogicalChannel></DMXChannel>
      <DMXChannel Offset="5"><LogicalChannel Attribute="Gobo1">
        <ChannelFunction Attribute="Gobo1" Wheel="Gobo1">
          <ChannelSet DMXFrom="0/1" WheelSlotIndex="1"/>
          <ChannelSet DMXFrom="10/1" WheelSlotIndex="2"/>
          <ChannelSet DMXFrom="20/1" WheelSlotIndex="3"/>
        </ChannelFunction></LogicalChannel></DMXChannel>
    </DMXChannels></DMXMode></DMXModes>
  </FixtureType>
</GDTF>
'@ | Set-Content -Path (Join-Path $tmp "description.xml") -Encoding UTF8

# Der Dateiname ist mit Absicht Unsinn: der Schlüssel kommt aus der Datei.
Compress-Archive -Path (Join-Path $tmp "description.xml") `
                 -DestinationPath (Join-Path $tmp "x.zip") -Force
Move-Item (Join-Path $tmp "x.zip") (Join-Path $dir "irgendwas-ganz-anderes.gdtf") -Force
```

**Danach PrismDMX neu starten** — die Bibliothek wird beim Start gelesen.

### Die Prüfungen

- [ ] **T-GDTF.1 — die Datei wird gelesen, und der Name ist egal.** View
      **Patch** → *Add fixture* → ins Suchfeld `rig test` tippen. Es steht eine
      Zeile da: **PrismDMX Test · Rig Test Light**, Spalte *Modes* `Mode 1`.
      Dass die Datei `irgendwas-ganz-anderes.gdtf` heißt, spielt keine Rolle —
      **der Schlüssel kommt aus dem Dokument**, nicht aus dem Dateinamen.
- [ ] **T-GDTF.2 — die neue Spalte *Format*.** In derselben Zeile steht in der
      Spalte **Format** das Wort **`GDTF`**, und in *Source* steht **`yours`**
      (die Datei liegt in Ihrem eigenen Ordner). Zum Vergleich: jede Zeile der
      Open-Fixture-Library-Profile liest **`OFL`**, Ihr eigenes
      Stairville-Profil ebenfalls **`OFL`** bei *Source* `yours`.
- [ ] **T-GDTF.3 — das Formular sagt, was das Profil mitbringt.** Die Zeile
      anklicken. Rechts, **unter** dem Namen des Fixtures, steht eine Zeile:
      **`3D model · 1 beam`**. Bei einem OFL-Profil (z. B. *Stage Wash*) steht
      dort **gar nichts** — keine leere Zeile, keine Null.
- [ ] **T-GDTF.4 — es patcht und hat die richtige Breite.** Universe `2`,
      Address `1`, **Patch**. Die neue Zeile belegt **5 Kanäle** (Pan 16 bit =
      2, Tilt, Dimmer, Gobo). In der Patch-Tabelle steht in der Spalte **`Ch`**
      eine **`5`**.
- [ ] **T-GDTF.5 — die Namen kommen vom Hersteller.** Fixture wählen
      (`Fixture N` + Enter). Im Programmer-Band:
      - Bank **Dimmer**: der Knopf heißt **`Dim`** — das ist das `Pretty`
        aus der GDTF-Datei, **nicht** das Wort *Dimmer* dieses Pults.
      - Bank **Gobo**: der Knopf heißt **`G1`**.
      - Bank **Position**: *Pan* und *Tilt*. **Pan ist 16 bit** (zwei Kanäle,
        `Offset="1,2"`), *Tilt* ist 8 bit — genau so steht es in der Datei
        oben, und genau daran sieht man, dass die Offsets gelesen wurden.
- [ ] **T-GDTF.6 — die Gobo-Stufen tragen die Namen des Rades.** Rechtsklick
      auf den Knopf **`G1`** (Bank *Gobo*): die Liste zeigt **`Open`**, **`Triangles`**,
      **`Dots`** — die Slots des Rades aus der Datei. (Die Bilder selbst zeigt
      noch nichts an; das ist S30s Arbeit, siehe `PROGRESS.md` §5.)
- [ ] **T-GDTF.7 — eine eigene `.gdtf` schlägt die installierte.** Nur sinnvoll,
      wenn in `profiles\fixtures\` eine Bibliothek liegt. In der obigen Datei
      `Name="Rig Test Light"` auf den Namen eines Fixtures ändern, das die
      installierte Bibliothek hat, Kanalzahl ändern, neu packen, neu starten:
      die Zeile steht **einmal** da, mit *Source* `yours`, und hat **Ihre**
      Kanalzahl.
- [ ] **T-GDTF.8 — kaputte Datei bringt das Pult nicht um.** Eine beliebige
      Datei nach `%APPDATA%\PrismDMX\fixtures\kaputt.gdtf` kopieren (z. B. ein
      Textdokument umbenennen). Neu starten: das Pult startet normal, die
      übrigen Profile sind alle da, und die kaputte taucht einfach nicht auf.
- [ ] **T-GDTF.9 — das alte Format läuft weiter.** Ihr eigenes Stairville-Profil
      (`fixtures\stairville\mh-z195-smart-white-cw-ww-a.json`) ist unverändert
      in der Liste, *Source* `yours`, *Format* `OFL`, und Fixture 1 der Show
      funktioniert wie vorher. **Das ist der wichtigste Punkt dieses Kapitels:**
      GDTF ist dazugekommen, es hat nichts ersetzt.
- [ ] **T-GDTF.10 — was der Daemon beim Start sagt.** Das Log geht auf
      **stderr**, es gibt *keine* Logdatei. Zum Mitlesen den Daemon einmal
      allein starten, in einer PowerShell, mit einem Wegwerf-Datenverzeichnis,
      damit Show und Rig unberührt bleiben:

      ```powershell
      $env:PRISMD_DATA_DIR = "$env:TEMP\prismd-scratch"
      & "$env:LOCALAPPDATA\PrismDMX\prismd.exe" --mock-output
      # ansehen, dann Strg+C
      ```

      Zwei Zeilen mit dem Ziel `library` müssen kommen: eine mit
      `... profiles from ... (N GDTF fixtures, ... beams, ... models; ...)` und
      eine mit `... profiles offered, N of them GDTF`. Ist eine Bibliothek
      installiert, in der **keine** GDTF liegt, kommt zusätzlich eine
      **Warnung**, dass sie keine Gobo-Bilder, Modelle und Beam-Geometrie hat.
      Danach `Remove-Item -Recurse $env:TEMP\prismd-scratch` und die
      PowerShell schließen (sonst bleibt `PRISMD_DATA_DIR` gesetzt).

---

## 1b. Neu — wie eine Bibliothek hereinkommt (S62)

**Wo:** View **Patch** (zwei neue Knöpfe) und *Settings* → **This machine** →
Abschnitt **GDTF Share**.

S61 hat das Pult GDTF lesen lassen; S62 baut die Wege, auf denen die Dateien
ankommen. Für diese Prüfungen brauchen Sie zusätzlich eine **`.mvr`** — jede
Planungssoftware exportiert eine, und wer keine hat, überspringt T-LIB.2 und
T-LIB.3 und vermerkt das unten.

**Schon vorab auf Windows geprüft** (beim Review vor dem Merge, PR #35), damit
Sie wissen, worauf es hier noch ankommt: der Windows-Speicher für Zugangsdaten
funktioniert — ein automatischer Test schreibt, liest über ein neues Handle
zurück, ersetzt und löscht einen Eintrag im **echten** Credential Manager (unter
einem eigenen Testnamen, Ihr Konto wird nie berührt); und das Pult erreicht
`gdtf-share.com` über TLS. **Nicht** geprüft werden konnte alles, wofür ein
echtes Konto nötig ist: der Login selbst und das Herunterladen (T-LIB.4/5).

### Die Prüfungen

- [ ] **T-LIB.1 — eine einzelne `.gdtf` über den Dateidialog.** View **Patch** →
      **Import profile (GDTF)** → die Datei aus §1 auswählen. Eine Notice sagt
      *Profile imported: …*, und die Zeile steht **sofort** im Auswahlfeld —
      ohne Neustart. Danach in `%APPDATA%\PrismDMX\fixtures\` nachsehen: die
      Datei liegt dort, unter ihrem eigenen Namen.
- [ ] **T-LIB.1b — was keine Fixture ist, kommt nicht in den Ordner.** Dasselbe
      noch einmal mit einem umbenannten Textdokument. Es kommt eine Notice, die
      sagt, dass die Datei keine lesbare Fixture ist, und in
      `%APPDATA%\PrismDMX\fixtures\` liegt sie **nicht**. Das ist die
      wichtigere Hälfte dieses Punktes.
- [ ] **T-LIB.2 — ein ganzes Rig, mit Patch.** View **Patch** → **Import rig
      (MVR)** → Ihre `.mvr` auswählen. Die Notice nennt vier Zahlen: wie viele
      Fixtures gepatcht wurden, wie viele Profile ankamen, und was übersprungen
      wurde. In der Patch-Tabelle stehen die geplanten Fixtures mit **ihren
      Nummern und Adressen aus der Planung**. Seit S30 sagt die Notice auch,
      wie viele **dort hängen, wo die Planung sie hat** (*… hung where the plan
      puts them*) — siehe T-BOTH.1.
- [ ] **T-LIB.3 — ein Oops nimmt den ganzen Import zurück.** Direkt nach T-LIB.2
      **einmal** Oops. **Alle** importierten Fixtures sind weg, die vorher
      gepatchten stehen unverändert da, und ein Redo bringt alles zurück. *Ein
      Import ist ein Schritt* — das ist die Zusage, und ein zweiter Oops-Druck,
      der nur die Hälfte zurücknähme, wäre der Fehler, auf den hier geachtet
      wird.
- [ ] **T-LIB.3b — der Betreiber gewinnt.** Wenn die `.mvr` eine Fixture-Nummer
      enthält, die Ihre Show schon benutzt: die vorhandene behält Nummer,
      Adresse und Profil, und die geplante bekommt die nächste freie Nummer. Die
      Notice zählt sie als *renumbered*.
- [ ] **T-LIB.4 — der erste echte Login.** ⚠️ **Das ist der Punkt, der
      hier nie getestet werden konnte** — der Entwicklungscontainer erreicht
      `gdtf-share.com` nicht, und der Dienst hat keinen anonymen Zugang. Ein
      kostenloses Konto anlegen, dann *Settings* → **This machine** → **GDTF
      Share**: Benutzername und Passwort eintragen, **ohne** den Haken, und
      **Update the library** drücken. Erwartet:
      - Die Zeile unter dem Formular sagt zuerst *Signing in and asking what is
        published…*, dann zählt sie (*412 of 3000 fixtures…*).
      - **Das Pult spielt dabei weiter Licht.** Einen Fader bewegen, während der
        Download läuft — er reagiert sofort. Das ist die eigentliche Prüfung.
      - Am Ende **ein Satz**, der sagt, wie viele Fixtures angekommen sind.
      - Die Profile stehen danach im Auswahlfeld, *Source* `yours`, *Format*
        `GDTF`.
      Schlägt der Login mit richtigen Zugangsdaten fehl, ist die wahrscheinliche
      Ursache in `docs/FIXTURE_LIBRARY.md` §3 beschrieben (form-encoded statt
      JSON) — **bitte den genauen Wortlaut der Meldung notieren**.
- [ ] **T-LIB.5 — die Zugangsdaten merken, und wieder vergessen.** Den Haken
      *Keep this account on this machine* setzen und noch einmal aktualisieren.
      Dann: das Pult neu starten, *Settings* → **This machine** → **GDTF Share**
      — es steht *Kept on this machine: <Ihr Name>*, und der Benutzername ist
      vorausgefüllt. In der **Windows-Anmeldeinformationsverwaltung**
      (`Systemsteuerung → Anmeldeinformationsverwaltung → Windows-Anmeldeinformationen`
      → *Generische Anmeldeinformationen*) steht ein Eintrag
      **`gdtf-share-account.PrismDMX — GDTF Share`** (so heißt er wirklich; in
      einer Eingabeaufforderung zeigt `cmdkey /list` ihn ebenso). Aufklappen:
      das Passwort ist **nicht** im Klartext zu sehen. Dann **Forget this
      account** drücken: die Zeile sagt wieder *No account is kept on this
      machine*, und der Eintrag in Windows ist **weg** (nach dem Schließen und
      Neuöffnen der Anmeldeinformationsverwaltung).
- [ ] **T-LIB.5b — ein falsches Passwort, gemerkt.** Haken setzen, absichtlich
      ein **falsches** Passwort, **Update the library**. Erwartet: ein Satz, dass
      das Konto abgelehnt wurde — und kein Absturz, kein hängender
      Fortschritt. **Bekannt:** gemerkt wird *vor* dem Login, also steht jetzt
      das falsche Passwort im Windows-Speicher. Danach mit dem richtigen
      Passwort noch einmal: der gemerkte Eintrag wird **ersetzt**, nicht
      verdoppelt (in der Anmeldeinformationsverwaltung steht weiterhin **ein**
      Eintrag). Ob das falsche Passwort gar nicht erst gemerkt werden sollte,
      ist eine Entscheidung für Sie — bitte vermerken.
- [ ] **T-LIB.6 — nichts davon steht in `machine.json`.** `%APPDATA%\PrismDMX\machine.json`
      öffnen und nach dem Passwort suchen. **Es darf nicht darin vorkommen** —
      auch nicht der Benutzername in einem Feld, das wie ein Passwort aussieht.
      Was dort stehen darf, ist nichts: der Kontoname kommt aus dem
      Windows-Speicher, nicht aus der Datei.
- [ ] **T-LIB.7 — ohne Konto ist alles trotzdem da.** *Forget this account*, das
      Pult neu starten, und im Auswahlfeld suchen: die Open-Fixture-Library-Profile
      sind da, Ihre eigenen Dateien sind da, und **nirgends steht eine
      Fehlermeldung** darüber, dass niemand angemeldet ist. Der Login ist ein
      Abschnitt, keine Tür.

---

## 2. Neu — das Controls-Menü (S59)

**Wo:** *Settings* → Tab **Controls**.

### Die Tastenwörter

- [ ] **T-S59.1 — es gibt einen Abschnitt *Console keys*.** In der Liste der
      Aktionen steht neben *Executors*, *The programmer* und *Other internal
      commands* ein Abschnitt mit den **23 Wörtern der Kommandozeile**:
      `Clear`, `Full`, `Update`, `Oops`, `Store`, `Edit`, `Goto`, `Move`,
      `Copy`, `Delete`, `Label`, `Color`, `Assign`, `New`, `Fixture`, `Group`,
      `Sequence`, `Cue`, `Preset`, `View`, `Executor`, `At`, `Thru`.
- [ ] **T-S59.2 — ein *append*-Wort hängt an die stehende Zeile an.** Eine freie
      Taste am X-Touch auf **`Fixture`** binden (Learn auf der Zeile, dann die
      Taste drücken). Kommandozeile leeren. Taste drücken → in der Zeile steht
      `Fixture `. Nochmal `1` tippen, dann eine auf **`Thru`** gebundene Taste
      → `Fixture 1 Thru `. **Die Zeile wird nicht ersetzt, es wird angehängt.**
- [ ] **T-S59.3 — ein *write*-Wort ersetzt die Zeile.** Eine Taste auf **`Store`**
      binden. Irgendetwas halb tippen, dann die Taste drücken → in der Zeile
      steht **nur** `Store ` und wartet.
- [ ] **T-S59.4 — ein *run*-Wort führt aus.** Eine Taste auf **`Clear`** binden.
      Etwas in den Programmer nehmen, Taste drücken → der Programmer leert sich,
      wie die Taste *CLEAR* auf dem Schirm.
- [ ] **T-S59.5 — zwei Tasten schnell hintereinander geben eine Zeile.** Die
      auf `Store` und die auf `Cue` gebundene Taste **so schnell wie möglich**
      nacheinander drücken. In der Zeile muss **`Store Cue `** stehen — nicht
      `Cue `. (Das war ein Fehler, den S59 beim Bauen gefunden hat: zwei Tasten
      in einem 30-Hz-Poll.)

### Die Lampen

- [ ] **T-S59.6 — eine gebundene Taste leuchtet, wenn ein Druck irgendwohin
      führt.** Zeile leer: die Taste auf `Store` leuchtet. Danach `1 ` tippen:
      die Taste auf **`Cue` ist dunkel** (nach einer Zahl nimmt die Zeile nur
      `at`, `thru`, `full`). `Clear` drücken, `Store ` schreiben: die Taste auf
      **`Cue` leuchtet**.
- [ ] **T-S59.7 — *Clear* folgt der Clear-Stufe, nicht dem Programmer.** Etwas
      in den Programmer nehmen. Die auf `Clear` gebundene Taste **dreimal**
      drücken. Nach dem **ersten** und nach dem **zweiten** Druck muss sie
      **noch leuchten** (Auswahl weg, Werte weg, aber Bank und Seite noch da);
      erst danach geht sie aus. **Das ist Ihre eigene Korrektur vom 2026-09-20.**
- [ ] **T-S59.8 — nichts leuchtet, was nichts tut.** Eine Taste auf ein Wort
      binden, das in der aktuellen Lage nichts bewirkt — sie bleibt dunkel. Das
      ist Absicht: dunkel ist eine zulässige Antwort.

### Das Jogwheel

- [ ] **T-S59.9 — der langsamste Klick bewegt genau einen DMX-Schritt.**
      Ein Fixture wählen, einen 8-Bit-Kanal auf einen Encoder legen, das
      Jogwheel **langsam** einen Rastpunkt weiter drehen. Der Wert muss sich um
      **genau 1** ändern — vorher war es ein Dreizehntel eines Schrittes, also
      praktisch nichts.
- [ ] **T-S59.10 — schnell ist viermal so weit.** Dasselbe Rad **schnell**
      drehen: pro Rastpunkt etwa **4** Schritte.
- [ ] **T-S59.11 — der Regler dafür.** *Settings* → **This machine** → Feld
      **Jog wheel**, in Prozent, 10–400 %. Auf `400` stellen, **Apply**. Das Rad
      wirkt **sofort beim nächsten Drehen**, ohne Neustart. Wieder auf `100`.
- [ ] **T-S59.12 — auch die langsamste Einstellung bewegt etwas.** Auf `10`
      stellen und drehen: der Wert ändert sich noch, nur wenig. Er darf **nie**
      bei null hängen bleiben. Zurück auf `100`.

### Das Menü selbst

- [ ] **T-S59.13 — die Strips sind aus der Liste raus.** In *Controls* gibt es
      unten einen zugeklappten Abschnitt **„Advanced: the strips, the main fader
      and the jog wheel"**. Die Fader, Executor-Buttons und Encoder der Strips
      sind **dort** und nicht mehr in der Hauptliste. Die Spalte
      *Strip / Selected* ist weg.
- [ ] **T-S59.14 — die Zeichnung des Pults.** Oben im Tab die beiden Knöpfe
      **List** und **The desk**. *The desk* drücken: es erscheint eine Zeichnung
      der X-Touch **maßstäblich**, jede Taste an ihrem Platz. Keine Taste liegt
      über einer anderen, nichts steht über den Rand hinaus.
- [ ] **T-S59.15 — die Zeichnung leuchtet mit.** Bei offener Zeichnung am
      echten Pult etwas tun, das eine Lampe ändert (z. B. `Store ` schreiben):
      die entsprechende Taste in der Zeichnung ändert sich mit.
- [ ] **T-S59.16 — 62 von 64 Tasten sind belegt.** In der Zeichnung oder der
      Liste: **SMPTE/Beats** ist frei (Ihr Weg zurück ans Tonpult) und
      **Name/Value** ist frei. Alle anderen Panel-Tasten tun etwas.
- [ ] **T-S59.17 — eine Seitenumschaltung lässt die Zeile stehen.**
      `Fixture 1 thru ` tippen (**nicht** Enter), dann am Pult die Executor-Seite
      umschalten. Die Zeile steht unverändert da.
- [ ] **T-S59.18 — Export und Import.** *Export* → Datei speichern. *Import* →
      dieselbe Datei → die Tabelle ist danach dieselbe.

---

## 2a. Neu — der 3D-Visualizer (S30 + S30b)

**Wo:** Fenster **Viewer 3D** (Rechtsklick auf eine leere Stelle des Canvas oder
`Einfg`, dann *Viewer 3D*). Am besten groß ziehen. Für T-3D.10 bis T-3D.15
braucht es einen **GDTF-Moving-Head** im Patch — das Robe-Robin-T1-Profil, mit
dem der erste Viewer abgelehnt wurde, ist genau richtig (in den Ordner
`%APPDATA%\PrismDMX\fixtures\` legen oder über **Import fixture (GDTF)**).

**Zuerst:** Die Zeile neben den Knöpfen sagt *… webgl*, wenn der Rechner in 3D
zeichnet. Steht dort nichts davon, zeichnet der Browser ohne 3D — dann ist
alles ab T-3D.10 nicht prüfbar; bitte vermerken.

- [ ] **T-3D.1 — das Rig ist da.** Alle gepatchten Fixtures erscheinen. Die Zeile
      neben den Knöpfen sagt *N fixtures · 0 lit · N not placed* — ein Rig, das
      nie platziert wurde, steht komplett im Ursprung.
- [ ] **T-3D.2 — platzieren.** `Fixture 1 Thru 4` + Enter (liegt eine Nummer dazwischen nicht im Patch,
      verweigert die Zeile `Thru` — dann `Fixture 1 + 2 + 5 + 7`).
      Rechts steht *4 fixtures: …*. **Y** = `6`, **Gap** = `2`, **Spread**. Die
      vier hängen nebeneinander in sechs Metern; *not placed* verschwindet aus der
      Zeile.
- [ ] **T-3D.3 — ein Oops.** `Oops` + Enter. **Alle vier** stehen wieder im
      Ursprung — die ganze Geste war ein Schritt. Nochmal **Spread**.
- [ ] **T-3D.4 — die Strahlen kommen vom Kabel.** `Fixture 1 Thru 4 At Full`. Aus
      allen vier kommt ein Strahl im Dunst, am Boden ein Lichtfleck — der Strahl
      endet **bündig** mit dem Fleck, nicht breiter und nicht schmaler; die Zeile
      sagt *4 lit*. Ein Executor mit einer Cue tut dasselbe: **was im DMX Sheet
      steht, steht auch hier**.
- [ ] **T-3D.5 — gleich nach dem Oops.** `Oops`, **sofort** danach `Fixture 1
      At Full` (beides schnell hintereinander, oder als Makro). Das Licht am Rig
      **und** im Viewer geht an. Vor S30b konnte ein Wert, der im selben Moment
      wie ein Neuaufbau des Rigs gesetzt wurde, auf dem Ausgang fehlen.
- [ ] **T-3D.6 — Rotation.** Ein Fixture wählen, **Rotation X** = `90`, **Set**:
      sein Strahl zeigt waagerecht zum Publikum. `180`: es steht auf dem Boden und
      strahlt nach oben.
- [ ] **T-3D.7 — umsehen.** Ziehen dreht, Shift-Ziehen verschiebt, Mausrad zoomt,
      *Front / Top / Side / 3D / Frame all* tun, was sie sagen. Ein zweiter
      Client (zweites Fenster, anderer Bildschirm) behält **seine** Ansicht.
- [ ] **T-3D.8 — klicken wählt.** Ein Klick auf ein Fixture im Bild nimmt es in
      die Auswahl (gelber Rand); nochmal klicken nimmt es heraus. Der gelbe Rand
      liegt **eng um das Gerät**, auch bei einer PAR-Kanne oder einem OFL-Gerät —
      nicht um den Strahl. Ein Klick in den Strahl neben dem Gerät wählt nichts.
- [ ] **T-3D.9 — der Ausgang merkt nichts.** Während *DMX Sheet* und *Viewer 3D*
      offen sind und eine Cue läuft: **im Viewer dauernd drehen** (Ziehen, eine
      halbe Minute). Das DMX Sheet zeigt weiter ~30 Hz, die Lampen am Rig
      flackern nicht, die Kommandozeile reagiert sofort.
- [ ] **T-3D.10 — das Gerät, nicht ein Kasten.** Der GDTF-Moving-Head ist als
      **sein eigenes Modell** zu sehen — Fuß, Bügel, Kopf —, nicht als Rechteck.
      Pan dreht den Bügel, Tilt den Kopf im Bügel, und der Strahl tritt **an der
      Linse** aus, nicht am Fuß und nicht daneben. *(Beantwortet die offene
      Frage der letzten Runde: die Verschiebung steht in der vierten Spalte der
      Matrix, in Metern — an einem echten Robin T1 geprüft.)*
- [ ] **T-3D.11 — Farben.** Am Moving Head Cyan, Magenta, Gelb einzeln und
      gemischt ziehen, dann das Farbrad (`colorwheel at …`) durch seine Slots
      und die Farbtemperatur (CTO) von kalt nach warm: der Strahl und der Fleck
      am Boden haben die Farbe. An einem LED-Gerät mit **Kaltweiß** und
      **Warmweiß** beide einzeln: Kaltweiß ist bläulich-weiß, Warmweiß gelblich.
- [ ] **T-3D.12 — Gobos.** `gobo 2 at …` (das erste Goborad; `gobo` ohne Zahl ist
      beim T1 das **Animationsrad**) durch die Slots: im Dunst und am Boden ist
      **das Bild des Gobos** zu sehen, nicht ein Kreis. Gobo-Rotation dreht es,
      ein drehendes Gobo dreht sich weiter, ohne dass jemand etwas tut.
- [ ] **T-3D.13 — Fokus, Frost, Iris, Zoom.** Fokus von einem Ende zum anderen:
      das Gobo wird scharf und wieder weich. Frost: der Strahl wird breiter und
      weich. Iris zu: der Strahl wird dünn. Zoom: schmal und breit.
- [ ] **T-3D.14 — Prisma und Blenden.** Prisma ein: der Strahl teilt sich in
      seine Facetten (beim T1 sechs). Eine Blende hinein (`blade at …`,
      `blade 3 at …`): der Fleck am Boden bekommt eine gerade Kante; die
      Blendendrehung dreht sie.
- [ ] **T-3D.15 — Strobe.** Shutter auf Strobe (am T1 `shutter at 30`): der
      Strahl blitzt, schneller mit höherem Wert; Puls-Bereiche blenden auf und ab.
- [ ] **T-3D.16 — Detail und Dunst.** **Detail** von *Low* bis *Ultra*: ab
      *Medium* die Modelle, bei *High* und *Ultra* feinere Strahlen und ein
      Glühen um helles Licht. Die Zeile zeigt, wie lange ein Bild braucht —
      **bitte für jede Stufe notieren** (mit der Zahl der Fixtures). **Haze** auf
      null: nur noch der Boden zeigt das Licht. Nach **jedem** Wechsel der Stufe
      ist das Bild unverzerrt (ein runder Fleck bleibt rund, ohne das Fenster
      anzufassen). Fenster schließen und wieder öffnen: beide Einstellungen sind
      noch da.
- [ ] **T-3D.17 — ein Profil ohne Gerätedaten.** Ein OFL-Moving-Head wird als
      einfacher Moving Head gezeichnet und folgt Pan und Tilt; eine PAR-Kanne
      als Kanne. Hat es ein Goborad, ist bei einem Gobo ein Muster im Strahl.

---

## 2b. Neu — beides zusammen: ein Rig aus der Planung im Viewer (S62 + S30)

**Wo:** View **Patch** und Fenster **Viewer 3D** nebeneinander. Braucht eine
`.mvr` wie in T-LIB.2; ohne eine überspringen und unten vermerken.

- [ ] **T-BOTH.1 — die Planung hängt, wo sie hängt.** Eine neue, leere Show
      anlegen, dann **Import rig (MVR)**. Im Viewer
      erscheinen die Fixtures **an ihren Plätzen aus der Planung**, nicht alle
      im Ursprung; *Frame all* zeigt das ganze Rig. Die Zeile neben den Knöpfen
      sagt *… not placed* nur für Fixtures, für die die Planung keinen Ort
      angibt. Grob vergleichen: stimmt die Höhe der Traverse (Y) und die
      Reihenfolge links/rechts mit dem Plan im Planungsprogramm?
- [ ] **T-BOTH.2 — ein Oops nimmt Patch und Orte zusammen zurück.** Direkt nach
      T-BOTH.1 **ein** Oops: im Viewer ist das Rig **ganz** weg, nicht nur
      verschoben. Redo: es hängt wieder an seinen Plätzen.
- [ ] **T-BOTH.3 — die Ausrichtung kommt noch nicht mit.** Alle importierten
      Fixtures hängen mit dem Strahl **nach unten**, auch wenn sie in der Planung
      gekippt oder gedreht sind — das ist bekannt und gewollt, bis die Richtung
      der MVR-Rotation an einer echten Datei geklärt ist (`PROGRESS.md` §5).
      **Bitte notieren**, welche Fixtures in Ihrer Planung eine Drehung haben und
      wie (z. B. *Seitenlicht links, 90° zur Bühne*): das ist genau die Angabe,
      mit der die Rotation eingebaut werden kann.
- [ ] **T-BOTH.4 — ein importiertes Profil wird richtig gezeichnet.** Ein
      Moving Head aus der `.mvr` ist im Viewer **sein eigenes Modell** (wie
      T-3D.10), sein Strahl tritt an der Linse aus und folgt Pan und Tilt. Das
      Gerät kommt aus der `.gdtf` **in** der `.mvr` — auch ohne dass es einzeln
      in der Bibliothek liegt.
- [ ] **T-BOTH.6 — das Pult startet mit der ganzen Bibliothek.** Mit der
      heruntergeladenen GDTF-Bibliothek (über 12 000 Fixtures) die PrismDMX-App
      beenden und neu starten. **Erster Start danach:** Oberfläche und
      Tray-Symbol sind nach wenigen Sekunden da; nach einer Weile (auf dem
      Entwicklungsrechner 19 s) meldet die Zeile unten *The fixture library is
      ready*. **Zweiter Start:** die ganze Bibliothek ist sofort da (Patch-Fenster,
      Suche nach einem Gerät). Im Task-Manager braucht `prismd` einige hundert
      MB, nicht Gigabytes. **Bitte beide Zeiten notieren.**
- [ ] **T-BOTH.7 — Import hält das Pult nicht an.** Während eine Cue läuft,
      *Import fixture (GDTF)*: das importierte Gerät ist sofort im
      Patch-Fenster, und das Pult reagiert in der Zeit danach ohne Pause.
- [ ] **T-BOTH.5 — die Bibliothek aktualisieren, während der Viewer läuft.**
      Mit Konto (T-LIB.4): *Update the library* drücken, während *Viewer 3D* und
      *DMX Sheet* offen sind und eine Cue läuft. Das DMX Sheet bleibt bei ~30 Hz,
      der Viewer dreht sich flüssig weiter, das Licht am Rig flackert nicht.

---

## 3. Was durch alle Sessions gehen musste

- [ ] **T-REG.1 — die Show öffnet unverändert.** `default.prism` öffnen: alle
      Fixtures, Cues, Gruppen, Presets und Executors sind da, mit denselben
      Werten. **Ein vor diesem Build gepatchtes Rig muss gleich aussehen** —
      S61 hat dem Profil ein Feld hinzugefügt, und ein Profil ohne dieses Feld
      muss sich verhalten wie vorher.
- [ ] **T-REG.2 — und sie lässt sich wieder speichern.** Etwas ändern,
      speichern, schließen, öffnen. Kein Fehler, nichts verloren.
- [ ] **T-REG.3 — Licht kommt an.** `1 at full` + Enter. Fixture 1 geht auf.
      `Clear`. Dasselbe für Fixture 2.
- [ ] **T-REG.4 — der Executor läuft.** Master-Fader von Executor 1 hoch, `Go`.
      Die Cue läuft, das Licht folgt.

---

## 4. Rauchtest — was schon vorher ging

Kurz, weil `docs/RELEASE_TEST.md` es in der vorigen Runde vollständig hatte.

- [ ] **T-RAUCH.1** Beide Ausgänge liefern (Art-Net-Node und Open DMX), *DMX
      Sheet* zeigt Werte.
- [ ] **T-RAUCH.2** Der Crossfade-Fader fährt nach dem Loslassen **nicht**
      zurück.
- [ ] **T-RAUCH.3** `Oops` bei stehender Zeile nimmt das letzte **Wort** weg,
      auch am X-Touch.
- [ ] **T-RAUCH.4** Ein Moduskanal benennt seinen Knopf um (B52), siehe
      `RELEASE_TEST.md` §2 — nur, falls Sie das Flat-Par-Fixture noch gepatcht
      haben.
- [ ] **T-RAUCH.5** Fenster wechseln, View wechseln, Vollbild mit `F11`.

---

## 5. Ergebnis

| | |
|---|---|
| Datum | |
| Build / Commit | |
| Befunde | |

**Kein Befund** heißt: 0.9.3 kann getaggt werden. Dann `docs/RELEASE_NOTES.md`
schreiben und `CHANGELOG.md` von *Noch nicht veröffentlicht* auf `0.9.3`
umstellen — der Tag `v0.9.3` fährt `release.yml` und baut den Installer mit
allen Gates, Windows eingeschlossen.

**Ein Befund** heißt: neue B-Nummer in `docs/ISSUES.md` oder ein GitHub-Issue,
und das Release wartet.
