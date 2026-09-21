# Abnahmetest am Test-Rig — Build 0.9.3 (S59 + S61)

**Gilt für:** den Build von `master` ab `5dd7112` — enthält **S59** (das
Controls-Menü, die Tastenwörter, die Lampen, das Jogwheel) und **S61** (die
Fixture-Bibliothek ist GDTF). Die Version im Installer ist weiterhin **`0.9.2`**:
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

- [ ] **T-0.1** Installer auf dem Rig ausführen. Eine installierte 0.9.2 wird
      dabei **überschrieben** — gleiche Versionsnummer.
- [ ] **T-0.2** Vor dem ersten Start nach `%APPDATA%\PrismDMX\` kopieren:
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

## 3. Was durch beide Sessions gehen musste

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
