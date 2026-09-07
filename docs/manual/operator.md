# PrismDMX — Handbuch für den Operator

**Für:** wer damit eine Show baut und fährt.
**Gilt für:** Version `0.9.2`. Welche Version Sie haben, sagt *Settings → This
machine*, und `prismd --version` sagt es auch.
**Sprache:** Deutsch. Die Beschriftungen auf dem Bildschirm sind englisch, und
sie stehen hier so, wie sie dort stehen — `Store`, `Clear`, *Fixture Sheet* —,
damit Sie das Wort im Text und das Wort auf dem Knopf nicht übersetzen müssen.

> **Offene Beta.** Dieses Programm ist vollständig genug, um ein Rig zu bauen,
> eine Show zu programmieren und sie auf echter Hardware zu fahren. Es hat noch
> keine Vorstellung in einem Haus gefahren, das nicht dem Autor gehört. Der
> Abschnitt [Wenn mitten in der Show etwas schiefgeht](#9-wenn-mitten-in-der-show-etwas-schiefgeht)
> steht deshalb weiter vorn, als er in einem 1.0-Handbuch stünde.

---

## Inhalt

1. [Was dieses Pult ist](#1-was-dieses-pult-ist)
2. [Der erste Start](#2-der-erste-start)
3. [Der Bildschirm](#3-der-bildschirm)
4. [Die Fenster](#4-die-fenster)
5. [Ein Rig patchen](#5-ein-rig-patchen)
6. [Auswählen und programmieren](#6-auswählen-und-programmieren)
7. [Speichern: Cues, Gruppen, Presets, Views](#7-speichern-cues-gruppen-presets-views)
8. [Wiedergabe](#8-wiedergabe)
9. [Wenn mitten in der Show etwas schiefgeht](#9-wenn-mitten-in-der-show-etwas-schiefgeht)
10. [Die Kommandozeile, Wort für Wort](#10-die-kommandozeile-wort-für-wort)
11. [Das X-Touch](#11-das-x-touch)
12. [Show-Dateien, Sicherung und Umzug](#12-show-dateien-sicherung-und-umzug)
13. [Was dieses Pult noch nicht kann](#13-was-dieses-pult-noch-nicht-kann)

---

## 1. Was dieses Pult ist

PrismDMX sind **zwei Prozesse, die als ein Programm ausgeliefert werden**.

- **`prismd`** ist das Pult. Ihm gehören die Show, die DMX-Ausgabe, das Timing
  und das Bedienpult. Er läuft weiter, ob jemand hinsieht oder nicht.
- **Die Oberfläche** ist ein Fenster. Sie zeichnet, was `prismd` sagt, und
  schickt ihm, was Sie drücken. Sie hält **nichts** selbst.

Die eine Konsequenz, die Sie im Betrieb merken: **das Fenster zu schließen
beendet die Show nicht.** Das Fenster verschwindet, das Licht bleibt, ein
Symbol im Infobereich sagt das. Ein zweiter Bildschirm, ein Laptop auf der
anderen Seite des Saals und die Fader am X-Touch sehen alle *dasselbe* Pult —
nicht Kopien davon.

Die zweite Konsequenz merken Sie, wenn etwas kaputtgeht: stürzt die Oberfläche
ab, hört die Ausgabe nicht auf. Genau dafür ist die Trennung da.

---

## 2. Der erste Start

**Installieren.** Windows 10 oder 11, 64 Bit. `PrismDMX_<version>_x64-setup.exe`
von der [Release-Seite](https://github.com/flakesystems/PrismDMX/releases)
herunterladen und ausführen. Das Installationsprogramm braucht **keine
Administratorrechte** und installiert in Ihr eigenes Profil — ein Schullaptop,
den ein Operator nicht verwalten darf, ist der Normalfall, und alles bis hin
zum Autostart funktioniert ohne erhöhte Rechte.

Windows wird beim ersten Mal *Der Computer wurde durch Windows geschützt*
zeigen. Der Build ist **nicht signiert**; das ist kein Urteil über die Datei,
sondern was Windows über jede ausführbare Datei sagt, die es selten gesehen
hat. *Weitere Informationen* → *Trotzdem ausführen*. Wenn Ihnen das zu weit
geht, ist das eine völlig vernünftige Haltung: prüfen Sie vorher die SHA-256 der
Datei gegen die auf der Release-Seite. Der ausführliche Weg steht im
[Installationshandbuch](installer.md).

**Starten.** *PrismDMX* im Startmenü. Der erste Start legt ein Datenverzeichnis
an, macht eine leere Show und die Identität dieses Pults und öffnet ein Fenster.

**Ein zweiter Start hängt sich an das Pult, das schon läuft.** Es gibt nie zwei
Engines. Zwei Pulte auf einem Rig ist der schlimmste Fehler, den dieses Programm
haben kann, und er ist strukturell verhindert, nicht durch Disziplin.

**Das Symbol im Infobereich** hat drei Einträge, und sie sagen, was sie tun:

| Eintrag | Was passiert |
|---|---|
| *Show the desk window* | Fenster zurückholen |
| *Close this window, leave the desk running* | Oberfläche beenden. Die Show läuft weiter |
| *Stop the desk* | Das Pult anhalten — **der Reihe nach**: Clients werden benachrichtigt, der eingestellte Blackout-oder-Halten kommt auf die Bühne, und die Ausgänge bekommen Zeit, ihn zu senden, bevor irgendetwas schließt |

**`F11` oder `Alt` + `Enter` schaltet Vollbild** und wieder zurück. Ein Pult im
Saal läuft ohne Titelleiste.

---

## 3. Der Bildschirm

Der Bildschirm ist als **Gerätebildschirm** gebaut und nicht als Webseite: außer
innerhalb eines Fensters wird nichts gescrollt. Was Sie sehen, ist alles, was da
ist.

```
 ┌──────────────────────────────────────────────────────────────┐
 │  Kopfzeile: Showname · Version · eine Lampe: antwortet das    │
 │             Pult?  ·  Add window  ·  View 1 View 2 …          │
 ├──────────────────────────────────────────────────────────────┤
 │                                                              │
 │   Die Leinwand: hier stehen die Fenster (Kapitel 4)          │
 │                                                              │
 ├──────────────────────────────────────────────────────────────┤
 │  Kommandozeile:  1 thru 6 at full             ⏎              │
 │  darunter: was an dieser Stelle erlaubt ist                  │
 ├──────────────────────────────────────────────────────────────┤
 │  Programmer-Band: Bank-Reiter · acht Encoder · Clear         │
 └──────────────────────────────────────────────────────────────┘
```

Vier Teile, und drei davon lohnen einen eigenen Satz.

**Die Lampe in der Kopfzeile** ist die eine Anzeige, die wahr sein muss, ohne
dass jemand ein Fenster geöffnet hat: *antwortet die Engine*. Alles andere —
Frames, Ausgänge, Sitzung — steht im Fenster *Status*.

**Die Kommandozeile ist die Bedienung.** Jede Taste, jede Kachel und jeder Knopf
auf diesem Bildschirm **schreibt eine Zeile**, statt selbst zu handeln. Was eine
Geste bedeutet, ist deshalb das, was die Zeile bedeutet — und jede Zeile lässt
sich auf eine Taste am X-Touch legen. Kapitel 10 ist die Wortliste.

**Was Sie gerade tippen, steht auf jedem Bildschirm.** Die Zeile gehört dem
Pult, nicht diesem Fenster. Was Sie *vorher* getippt haben, gehört Ihnen: die
Pfeiltasten hoch und runter laufen durch Ihre eigene Historie, und zwei
Operatoren an zwei Bildschirmen haben jeder ihre eigene.

**Das Programmer-Band** ist unten und immer da: die sieben Bank-Reiter, acht
Encoder und die `Clear`-Taste. Es zeigt **nur Parameter, die die ausgewählten
Fixtures wirklich haben** — ein vierfarbiger PAR sind vier Knöpfe und keine
Bank, durch die Sie blättern.

---

## 4. Die Fenster

Fenster kommen mit **Add window** in der Kopfzeile auf die Leinwand, oder mit
einer F-Taste am X-Touch. Das Pult sucht selbst einen freien Platz; Fenster
überlagern sich nicht. Ziehen und Größe ändern gehört diesem Bildschirm allein —
eine Zeile ist es nicht, weil es kein Wort dafür gibt.

**Ein Layout ist ein View.** `Store View 2 "Programmieren"` legt die geöffneten
Fenster mit ihren Rechtecken ab, `View 2` holt sie zurück, und weil ein View
Sitzungszustand ist, schaltet er auf **jedem** angehängten Bildschirm um — auch
vom X-Touch aus, ohne dass ein Client läuft.

Vierzehn Fenstertypen, und diese Tabelle wird aus dem Code erzeugt: sie kann
nicht veralten, ohne dass ein Test rot wird.

<!-- generated:window-types -->
| Fenster | Wofür |
|---|---|
| `FixtureSheet` | *Fixture Sheet* — der **Zustand**: pro Fixture und Parameter, was der Programmer hält und was auf dem Kabel liegt |
| `DmxSheet` | *DMX Sheet* — das **Kabel**: Universe für Universe, Kanal für Kanal, ganz ohne Fixtures |
| `SequenceSheet` | *Sequence Sheet* — die **Cue-Liste**: welche Sequenzen es gibt, welche gerade gilt, und ihre Cues mit Nummer, Name, Zeiten und Trigger |
| `Groups` | *Groups* — der **Gruppen-Pool**: Listen von Fixtures, nicht Looks. Das ist, was `Group 3` zu einer Auswahl macht |
| `Viewer3D` | *Viewer 3D* — die 3D-Bühnenansicht. **In diesem Build nicht gebaut**; das Fenster sagt das, statt leer zu bleiben |
| `PhaserEditor` | *Phaser Editor* — der Effekt-Editor. **In diesem Build nicht gebaut**; es gibt noch keine Effekt-Engine dahinter |
| `ClockViewer` | *Clock Viewer* — die Uhr, groß genug, um sie von hinten zu lesen, und was gerade läuft. Timecode-Spalten fehlen, weil es keinen Timecode gibt |
| `CueViewer` | *Cue Viewer* — die **Cue**: was sie setzt, Fixture für Fixture, mit den Preset-Verweisen. Zum Ansehen, nicht zum Bearbeiten |
| `PresetPool` | *Preset Pool* — die **Pools**: benannte Looks je Kategorie, mit der Farbe, die die Scribble-Strips zeigen |
| `Patch` | *Patch* — das **Rig**: welche Fixtures es gibt und wo ihre Kanäle liegen. Das einzige Fenster, das die Gestalt der Show ändert |
| `Settings` | *Settings* — das **Pult**: Ausgänge, Geräte, Tastenbelegung, Show-Dateien und diese Maschine. Kapitel 5 des [Installationshandbuchs](installer.md) |
| `Executors` | *Executors* — der **Strip**: die acht Executors der aktuellen Seite, ihre Fader und ihre je vier Tasten, und der Editor dafür |
| `CommandKeys` | *Command Keys* — die Wörter der Kommandozeile als Knöpfe. Wer die Wörter kann, schließt das Fenster; wer sie lernt, lässt es offen |
| `Status` | *Status* — die Messwerte: die Show, die Sitzung, die Engine und die Ausgänge |
<!-- /generated -->

### Fixture Sheet · `FixtureSheet`

Eine Zeile je Fixture, eine Spalte je Parameter. Zwei Werte stehen
übereinander: was der **Programmer** hält (Ihre laufende Änderung) und was
tatsächlich **auf dem Kabel** liegt. Die beiden auseinanderzuhalten ist der
ganze Zweck des Fensters — ein Wert, den Sie gesetzt haben, und ein Wert, den
eine Cue gerade fährt, sehen auf einem Pult ohne diese Trennung gleich aus.

Ein Klick auf eine Zeile wählt das Fixture aus. Steht ein Verb in der Zeile,
hängt derselbe Klick stattdessen sein Wort an — siehe Kapitel 10.

### DMX Sheet · `DmxSheet`

Was das Haus wirklich bekommt: 512 Kanäle je Universe, ohne jede Interpretation.
Das ist das einzige Fenster, das dem *Fixture Sheet* widersprechen kann, und
genau deshalb gibt es beide. Beim Patchen ist es das Fenster, das Ihnen sagt,
ob eine Adresse wirklich da ankommt, wo Sie sie vermuten.

### Sequence Sheet · `SequenceSheet`

Die Cue-Listen. Oben, welche es gibt und welche **ausgewählt** ist — das ist die,
in die `Store Cue 5` speichert. Darunter deren Cues: Nummer, Name, Fade, Delay,
Trigger. Ein Klick auf eine Cue-Zeile ist eine Auswahl; mit `Goto` oder `Edit` in
der Zeile ist derselbe Klick das fehlende Argument.

### Groups · `Groups`

Der Gruppen-Pool. Eine Gruppe ist eine **Liste von Fixtures**, kein Look: `Group
3` wählt aus, es setzt nichts. `Store Group 3 "Frontlicht"` legt die aktuelle
**Auswahl** ab.

### Viewer 3D · `Viewer3D`

Noch nicht gebaut. Das Fenster öffnet sich und sagt das in einem Satz — und das
ist Absicht: wer es von einer F-Taste am X-Touch aufmacht, soll eine Antwort
finden und kein leeres Rechteck, über das man einen Fehlerbericht schreibt.

### Phaser Editor · `PhaserEditor`

Ebenfalls noch nicht gebaut; dahinter fehlt die Effekt-Engine, nicht das
Fenster.

### Clock Viewer · `ClockViewer`

Die Uhrzeit, groß, und was gerade läuft. Was fehlt, ist die Timecode-Hälfte, und
das Fenster nennt die fehlenden Spalten selbst, statt sie leer zu zeigen.

### Cue Viewer · `CueViewer`

Was die laufende Cue eines Executors tatsächlich setzt, Fixture für Fixture, und
wo ein Wert aus einem Preset kommt. Ein Fenster zum Hinsehen: bearbeitet wird
mit `Edit Cue 3`, was die Cue in den Programmer lädt.

### Preset Pool · `PresetPool`

Acht Pools: die sieben Kategorien und **Multi**. Ein Preset aus einer Kategorie
enthält nur deren Werte — eine Farbe, eine Position —, ein `Multi`-Preset alles,
was der Programmer hält. Der Pool ist das, wo ein *fertiger Look* abgelegt
wird, die Kategorien sind die *Zutaten*.

Die Farbe, die Sie einem Preset geben, ist die, die die Scribble-Strips am
X-Touch leuchten. Ein Strip hat drei Lampen, also gibt es sieben Wörter dafür —
Kapitel 10.

### Patch · `Patch`

Das Rig: welche Fixtures existieren, welches Profil, welche Nummer, welche
Adresse. Kapitel 5.

### Settings · `Settings`

Fünf Reiter: *Outputs*, *Devices*, *Controls*, *Show files*, *This machine*. Es
ist ein **Fenster** und kein Dialog: es liegt auf der Leinwand wie jedes andere,
die Show läuft dahinter weiter, und Sie können es auf einem zweiten Bildschirm
offen lassen. Alles darin ist im [Installationshandbuch](installer.md)
beschrieben, außer *Show files*, das in Kapitel 12 steht.

### Executors · `Executors`

Die acht Executors der aktuellen Seite: je ein Fader, ein Encoder und vier
Tasten, und der Editor, der sagt, was die tun. Kapitel 8.

### Command Keys · `CommandKeys`

Die Wörter der Kommandozeile als Knöpfe. Jeder davon schreibt sein Wort in die
Zeile — er tut nichts Eigenes. Deshalb ist es dasselbe, ob Sie `Store` tippen
oder drücken.

### Status · `Status`

Die Messwerte, an einer Stelle: welche Show offen ist und ob sie ungespeicherte
Änderungen hat, wie viele Clients hängen, mit welcher Rate die Engine tickt und
wie es jedem Ausgang geht. Bei einem Netzwerk-Ausgang steht darunter, welche
Node antwortet und welche nicht — ein konfigurierter Ausgang auf ein leeres Rack
liest **Degraded** und nicht *OK*.

---

## 5. Ein Rig patchen

In der Reihenfolge, in der es üblicherweise gemacht wird:

**1. Sagen, womit das Haus verkabelt ist.** *Settings → Outputs*. Das ist die
Arbeit des Installateurs und steht in dessen [Handbuch](installer.md); wenn Sie
in einem eingerichteten Haus sind, ist es schon da.

**2. Sagen, was daran hängt.** Das Fenster *Patch*. Die Fixture-Bibliothek wird
nach Namen durchsucht; jedes Fixture bekommt eine **Nummer** (die, die Sie auf
der Kommandozeile tippen) und eine **Adresse** (Universe und Startkanal).

Das Pult sagt Ihnen **vor** dem Absenden, ob eine Adresse mit einer anderen
kollidiert. Eine Überlappung ist erlaubt — zwei Fixtures auf einer Adresse ist
manchmal genau das, was man will —, aber sie wird benannt, statt später
aufzufallen.

**3. Nachsehen, ob es ankommt.** *DMX Sheet* öffnen, das Fixture auf voll
ziehen, hinsehen. Wenn dort nichts passiert, liegt es an den Ausgängen und nicht
am Patch.

### Eigene Fixture-Profile

Eine Lampe, die die Open Fixture Library nicht kennt, ist ein Profil, das Sie
selbst schreiben können. Es gehört in **`fixtures/` im Datenverzeichnis des
Pults** — unter Windows `%APPDATA%\PrismDMX\fixtures` —, im JSON-Format der Open
Fixture Library, und wird beim Start gelesen.

Dieses Verzeichnis und **nicht** `profiles/fixtures/`: das zweite ist ein
**Download**, den `tools/fetch-fixtures` bei jedem Lauf leert. Das
Datenverzeichnis rührt kein Installationsprogramm an.

| Wohin | Wofür |
|---|---|
| `fixtures/meine-lampe.json` | Eine Lampe, für die es kein Profil gibt. Sie steht unter *Custom* neben allem anderen im Auswahlfeld |
| `fixtures/<hersteller>/<fixture>.json` | Eine **Korrektur** an einem mitgelieferten Profil. Gleicher Herstellerordner, gleicher Dateiname wie in der Bibliothek — Ihres ersetzt es |

Die Spalte *Source* im Auswahlfeld markiert Ihre Profile als **yours**. Und
sobald Sie damit gepatcht haben, wird das Profil **in die Show kopiert**: eine
`.prism`-Datei ist in sich vollständig und öffnet auf einem Pult, das Ihr
Verzeichnis nie gesehen hat, genauso.

---

## 6. Auswählen und programmieren

**Auswählen** geht auf drei Wegen, und alle drei sind dieselbe Zeile:

```
1 thru 6            Fixture 1 bis 6
1 + 3               1 und 3  ( "," bedeutet dasselbe )
Group 3             die Fixtures der Gruppe 3
```

Ein Klick auf eine Fixture-Zeile im *Fixture Sheet* oder auf eine Gruppen-Kachel
tut dasselbe. `1thru4` ohne Leerzeichen ist auch ein Bereich — eine Pult-Tastatur
hat keine Leertaste, die man gern trifft.

**Werte setzen** geht mit `at`, mit den Encodern oder mit dem Jogwheel:

```
at 50               die Auswahl auf 50 %
1 thru 4 at 50      auswählen und setzen — zwei Befehle in einer Zeile
5 pan at 25         ein anderer Parameter als der Dimmer
at full · at out    die Wörter für die beiden Enden
Full                die Auswahl auf voll, sonst nichts in der Zeile
```

### Die Encoder-Bänke

Sieben Bänke — *Dimmer*, *Position*, *Gobo*, *Color*, *Beam*, *Focus*,
*Control* — und darauf **nur, was die ausgewählten Fixtures haben**. Wählen Sie
einen vierfarbigen PAR aus, hat die Farbbank vier Knöpfe.

**Jeder Knopf trägt den Namen, den der Hersteller diesem Kanal gegeben hat** —
*Rotating Gobo*, *Color Wheel 2* — und nicht das Allgemeinwort des Pults. Wählen
Sie zwei Köpfe aus, die denselben Knopf verschieden nennen, kommt das
Allgemeinwort zurück, weil einer der beiden Namen dann falsch wäre.

**Ein Rad steht auf der Bank, die sein Inhalt sagt**: ein Rad voller Farben liegt
unter *Color*, auch wenn das Profil es anders nennt.

### Ein Fixture mit zwei Parametern derselben Art

Ein Kopf mit zwei Farbrädern, eine Röhre mit einem Rot je Pixel, ein Weißlicht
mit Warm- und Kaltweiß: solche Kanäle werden **durchnummeriert**.

- Auf der Bank stehen sie nebeneinander: *Gobo*, *Gobo 2*.
- Auf der Zeile: `1 gobo 2 at 50` — die **zweite** dieser Art an Fixture 1. Die
  Zählung beginnt bei eins, also sind `gobo 1` und `gobo` dasselbe Rad.
- Sind es mehr, als nebeneinander passen — eine Röhre mit vierundzwanzig Pixeln
  —, bekommt das Band stattdessen einen **Part**-Schalter und zeigt einen Teil
  auf einmal.

Die Regel für die Zeile ist absichtlich eng, denn das Kürzeste, was eine
Ordnungszahl sein könnte, ist auch eine Fixture-Nummer: sie gilt nur, wenn das
Parameterwort **nicht das erste Wort** der Zeile ist und die Zahl **direkt vor
`at`** steht. `pan 5 at 25` wählt weiterhin Fixture 5 aus.

### Vordefinierte Stufen

Ein Gobo-Rad, ein Farbrad oder ein Effektkanal ist im Profil eine Liste
benannter Stellungen. **Rechtsklick auf den Encoder** öffnet genau diese Liste,
unter den Namen des Herstellers, und schreibt beim Auswählen die Mitte der
Stellung. Ein Encoder ohne Stufen öffnet nichts.

### Jeder Kanal hat einen Knopf

Ohne Ausnahme. Wo das Profil sagt, was ein Kanal tut, ist der Knopf das — ein
Rot, ein Gobo-Rad, eine Iris. Wo es das nicht sagt, oder wo die Bedeutung eines
Kanals vom Wert eines anderen abhängt, gibt es den Knopf trotzdem: auf der Bank
**Control**, benannt, wie das Profil ihn nennt, oder `Ch 7` — die Stelle des
Kanals im Fixture, von eins gezählt. Von der Zeile aus: `1 raw 3 at 50` fährt den
dritten solchen Kanal von Fixture 1 auf die Hälfte.

Zwei Dinge, die dabei **nicht** gelten und die Sie wissen sollten:

- So ein Knopf hat **keine benannten Stufen**. Es gibt nichts, woraus sie zu
  lesen wären.
- Bei einem Kanal, dessen Bedeutung von einem Moduskanal abhängt, **folgt das
  Pult dem Umschalten nicht, während die Show läuft**. Drehen Sie den Modus,
  heißt der Nachbarknopf weiter, wie er hieß. Das ist bekannt und steht als
  `B52` in [`../ISSUES.md`](../ISSUES.md).

### `Clear` lässt los, bevor es vergisst

Drei Stufen, und die Taste sagt selbst, welche der nächste Druck wäre:

| Druck | Was passiert |
|---|---|
| 1 | Die **Auswahl** fällt weg, die Werte bleiben stehen. Das nächste Fixture kommt zum selben Look dazu |
| 2 | Die **Werte** fallen weg |
| 3 | Encoder-Bank und Seite gehen auf Anfang zurück |

So bauen Sie einen Look aus mehreren Fixtures: auswählen, setzen, einmal
`Clear`, das nächste auswählen, setzen. Der erste Wert steht noch.

---

## 7. Speichern: Cues, Gruppen, Presets, Views

```
Store Cue 5                     in Cue 5 der ausgewählten Sequenz
Store Sequence 5 Cue 3          in eine Cue einer benannten Liste
Store Sequence 4                in Cue-Liste 4 — und legt sie an, wenn die Nummer frei ist
Store Preset 1                  in Preset 1, in der Bank, die die Encoder gerade zeigen
Store Preset 1 Color            in einen benannten Pool
Store Preset 1 Multi "Der Look"  alles, was der Programmer hält, über die Bänke hinweg
Store Group 3 "Frontlicht"      die Auswahl als Gruppe 3
Store View 2 "Programmieren"    die Leinwand als View 2
```

**Ist das Ziel belegt, fragt die Zeile** — *merge*, *override* oder *cancel*,
in der Kommandozeile selbst und nicht in einem Fenster über der Leinwand. Die
Show läuft weiter, während die Frage steht: kein anderer Bildschirm ist
blockiert, und ein `Go` vom X-Touch wartet nicht darauf. `Escape` bricht ab, und
ein abgebrochener Dialog ändert **gar nichts**.

Eine Cue-Liste wird stattdessen mit *append*, *override* oder *merge* gefragt,
weil es beim Speichern einer Sequenz um **Cues** geht und beim Speichern einer
Cue um **Werte**.

**Eine auf einer freien Nummer neu angelegte Cue-Liste wird die ausgewählte.**
`Store Cue 1` nennt keine Liste und meint die ausgewählte — ein Pult, das Liste 4
anlegt und weiter auf Liste 1 zeigt, schickte den nächsten Store an die falsche
Stelle. In eine Liste zu speichern, die es schon gibt, ändert die Auswahl
dagegen nicht: was Sie bearbeiten, sagt `Sequence 4`.

**`Update`** speichert den Programmer in die Cue zurück, aus der er geladen
wurde, im Override-Modus. Die Taste **blinkt**, wenn es etwas zurückzuschreiben
gibt — und weil das Sitzungszustand ist, blinkt sie auf jedem Bildschirm
gleichzeitig.

**`Oops`** nimmt die letzte Änderung zurück. Zweihundert Schritte weit.

---

## 8. Wiedergabe

**Eine Cue-Liste auf einen Fader legen:**

```
Assign Sequence 5 Executor 1
```

Danach fahren die Tasten und der Fader von Executor 1 diese Liste. `Page 2`
blättert die Fader-Bank.

**Eine Cue-Liste, die auf keinem Fader liegt, spielt trotzdem** — und es ist
*dieselbe* Wiedergabe, die ein Fader fahren würde. `On Sequence 1` und ein `Go`
auf dem Executor, auf den sie später jemand legt, sind eine Wiedergabe mit einem
Cue-Zeiger. Zwei Executors auf einer Liste sind zwei **Griffe**, nicht zwei
Spieler: beide `Master`-Fader zeigen denselben Wert, und einer zu ziehen bewegt
beide.

**Transport:**

```
Go+ · Go-        vor und zurück, auf der ausgewählten Liste
On · Off         starten und stoppen
Go+ Executor 1   auf einem Executor
Go+ Sequence 2   auf einer Cue-Liste, wo immer sie spielt
Goto Cue 5       direkt auf Cue 5 springen
```

Eine Cue-Liste **landet an derselben Stelle**, ob Sie zu einer Cue gelaufen sind
oder hingesprungen: was eine Cue nicht selbst sagt, wird aus der Liste
gerechnet und nicht aus dem angesammelt, wo die Wiedergabe gerade war.

### Was ein Executor kann, und wie man es ändert

Jeder Executor hat **einen Fader**, **einen Encoder** und **vier Tasten**, und
was die tun, ist einstellbar — im Fenster *Executors* oder auf der Zeile:

```
Assign Executor 1 Fader Master     Empty · Master · Speed · XFade · Fade
Assign Executor 1 Encoder Speed    Empty · Master · Speed
Assign Executor 1 Button 2 Go+     Empty · Go+ · Go- · LearnSpeed · Off · On ·
                                   Flash · Toggle — die vier Tasten sind
                                   von eins durchnummeriert
Assign Executor 1 Button 4 Command "Go+ Sequence 3"
```

Die letzte Zeile ist die interessante: eine Taste kann **eine Zeile schicken**.
Damit ist alles, was Sie tippen können, auch eine Taste — und dieselbe Zeile
lässt sich auf eine X-Touch-Taste legen, sodass Fenster, Zeile und Hardware ein
Weg sind und nicht drei.

> Eine Zeile mit Satzzeichen darin **gehört in Anführungszeichen**. `go+`, `+`
> und `,` werden vom Zerleger umgeschrieben, damit `1 + 2` das heißt, was da
> steht. Also: `Command "Go+ Sequence 3"`, nicht `Command Go+ Sequence 3`.

### Ein Crossfade-Fader wird gelaufen, nicht zurückgesetzt

Der Fader eines Executors kann eines von zwei Crossfades sein:

| Modus | Hoch | Runter |
|---|---|---|
| **XFade** | blendet von der laufenden Cue auf die nächste | von dieser auf die übernächste |
| **Fade** | blendet die laufende Cue aus | blendet die nächste ein |

In beiden Fällen laufen Sie eine Cue-Liste, indem Sie **einen Fader hoch und
runter bewegen, ohne die Hand zu heben**. Auf halbem Weg stehen zu bleiben, hält
die Mischung auf halbem Weg — genau dafür hat man einen Crossfade auf einem
Fader.

**Das Pult bewegt einen Crossfade-Fader nie selbst.** Es gibt keinen
Rücksprung, auf keinem Bildschirm und an keinem Motorfader.

---

## 9. Wenn mitten in der Show etwas schiefgeht

Der Reihe nach, und die Reihenfolge ist wichtig.

### Das Bild ist eingefroren, das Licht steht

**Wahrscheinlich ist die Oberfläche weg, nicht das Pult.** Sehen Sie auf die
Lampe in der Kopfzeile. Ist sie aus oder ist das Fenster gar nicht mehr da:

1. **Nichts an der Bühne ändert sich davon.** `prismd` läuft weiter und sendet
   weiter DMX. Der letzte Look steht.
2. *PrismDMX* aus dem Startmenü noch einmal starten. Es **hängt sich an das Pult
   an, das schon läuft** — es startet kein zweites. Sie bekommen dasselbe Bild
   zurück, mit derselben Show, denselben Fadern und derselben Cue-Position.
3. Ist das Fenster wieder da, machen Sie weiter. Es ist nichts verloren
   gegangen: das Fenster hielt ohnehin nichts.

### Das Licht steht, aber nichts reagiert mehr

Dann ist es das Pult und nicht die Oberfläche. Sagt das Symbol im Infobereich
*the desk has stopped*, ist `prismd` beendet worden. Starten Sie das Programm neu
— die Show wird aus der zuletzt geöffneten Datei geladen; alles, was seit dem
letzten Speichern war, steht in der **Wiederherstellungskopie** neben der Show
(Kapitel 12).

### Licht ist an, das niemand programmiert hat

Das ist der Fall, der zuerst gemeldet gehört. Bevor Sie melden:

1. **Blackout ist keine Taste hier** — der Weg ist `Off` auf der laufenden Liste
   oder der Grand Master auf null.
2. *DMX Sheet* öffnen und nachsehen, **welche** Kanäle das sind. Steht dort
   etwas und im *Fixture Sheet* nicht, kommt es nicht aus der Show.
3. Ist ein zweites Pult im Haus auf denselben Universen? Zwei Sender auf einem
   sACN-Universe ist der häufigste Fall dieser Sorte, und keiner der beiden
   merkt es.

### Ein Ausgang ist weg

Das Fenster *Status* sagt, welcher. Ein USB-Adapter, den jemand gezogen hat, ist
der **erwartete** Fall: der Treiber versucht es weiter, und ein wieder
eingestecktes Kabel wird ohne Neustart aufgenommen. Ein Netzwerk-Ausgang, der
**Degraded** liest, hat entweder keine antwortende Node oder das Pult hört nicht
zu; darunter steht, welche der beiden Möglichkeiten es ist.

### Bevor die Vorstellung anfängt

- **Speichern.** `Ctrl-S`, oder *Settings → Show files*.
- **Nachsehen, ob ein Universe ins Leere geht.** *Settings → Outputs* nennt die
  gepatchten Universen, die kein Ausgang trägt. Das vor der Show zu lesen, ist
  billiger, als es an einer Lampe zu merken, die nicht angeht.
- **Einen Weg zurück haben.** Ein zweites Pult, ein Saallichtschalter in
  Reichweite, oder eine Probe, die Sie abbrechen können. Diese Bitte steht hier,
  weil die Version eine Beta ist.

---

## 10. Die Kommandozeile, Wort für Wort

Groß- und Kleinschreibung ist egal. Leerzeichen meistens auch. Ein Name darf in
Anführungszeichen stehen und muss nicht.

Unter dem Eingabefeld steht, **was an dieser Stelle der Zeile erlaubt ist** —
die Wörter, nie die Nummern. `Tab` nimmt das erste. Es ist eine Antwort der
**Grammatik** und nicht der Show: es bietet das Wort `sequence` an, nie die
Sequenzen, die es gibt. Was es gibt, dafür sind die Pools auf der Leinwand da.

Diese Liste wird aus dem Code erzeugt — aus `prism_core::console::CONSOLE_WORDS`,
derselben Tabelle, aus der die Vervollständigung liest.

<!-- generated:console-words -->
| Wort | Was es tut |
|---|---|
| `assign` | Legt eine Cue-Liste auf einen Executor (`Assign Sequence 5 Executor 1`) oder sagt, was ein Fader, ein Encoder oder eine der vier Tasten tut (`Assign Executor 1 Fader XFade`). Welche der beiden Bedeutungen gilt, entscheidet das erste Hauptwort |
| `at` | Setzt einen Wert: `at 50`, `at full`, `at out`, `5 pan at 25` |
| `clear` | Die drei Stufen aus Kapitel 6: erst die Auswahl, dann die Werte, dann Bank und Seite |
| `color` | Gibt einem Objekt eine Farbe für den Scribble-Strip: `Color Sequence 4 blue`. Ohne letztes Wort **nimmt es die Farbe weg** |
| `copy` | Kopiert ein Objekt auf eine andere Nummer: `Copy Sequence 2 Sequence 6`. Ebenso für Cues, Gruppen, Presets und Views |
| `cue` | Das Hauptwort für eine Cue. Allein ist es **keine Zeile** — `Cue 5` könnte ein Goto oder ein Edit sein, und das Pult sagt das, statt zu raten |
| `delete` | Löscht ein Objekt: `Delete Cue 3` |
| `edit` | Lädt eine Cue in den Programmer, um sie zu ändern: `Edit Cue 3`, `Edit Sequence 5 Cue 3` |
| `executor` | Das Hauptwort für einen Executor. Allein — `Executor 3` — wählt den, auf den der Transport wirkt |
| `fixture` | Das Hauptwort für ein Fixture, für alle, die es aus Gewohnheit tippen: `Fixture 12 thru 16` ist dasselbe wie `12 thru 16` |
| `full` | Die Auswahl auf voll, ohne dass sonst etwas in der Zeile steht |
| `go` | Startet die Wiedergabe. `go 3` ist ein Executor — das war es, als es nichts anderes sein konnte, und es gilt weiter |
| `go+` | Eine Cue vor: auf der ausgewählten Liste, oder auf der benannten (`Go+ Sequence 2`) |
| `go-` | Eine Cue zurück, mit denselben Formen |
| `goto` | Springt direkt auf eine Cue: `Goto Cue 5`, `Goto Sequence 2 Cue 5` |
| `group` | Das Hauptwort für eine Gruppe. Allein — `Group 3` — wählt deren Fixtures aus |
| `label` | Benennt ein Objekt: `Label View 1 "Programmieren"`. Ohne letztes Wort **nimmt es den Namen weg** |
| `move` | Nummeriert um: `Move Cue 3 Cue 8`. Bei Executors und Views wird **getauscht**, wenn das Ziel belegt ist |
| `new` | Legt einen leeren View an und schaltet darauf: `New View 3 "Fahren"` |
| `off` | Hält eine Wiedergabe an. `off 3` ist ein Executor |
| `on` | Startet eine Wiedergabe, ohne eine Cue weiterzuschalten |
| `oops` | Nimmt die letzte Änderung zurück |
| `page` | Blättert die Fader-Bank: `Page 2` |
| `preset` | Das Hauptwort für ein Preset. Allein — `Preset 4` — wendet es auf die Auswahl an |
| `sequence` | Das Hauptwort für eine Cue-Liste. Allein — `Sequence 5` — macht sie zu der, in die gespeichert wird |
| `store` | Speichert: Cue, Sequence, Preset, Group oder View. Kapitel 7 |
| `thru` | Ein Bereich, in der Richtung, in der er geschrieben steht: `1 thru 4`, `6 thru 2` |
| `update` | Speichert den Programmer in die Cue zurück, aus der er geladen wurde |
| `view` | Das Hauptwort für einen View. Allein — `View 2` — schaltet die Leinwand darauf |
<!-- /generated -->

### Ein Pool ist eine Taste

Steht ein **Verb** in der Zeile, hängt ein Klick auf eine Kachel sein Wort an,
statt auszuwählen. `Store` und ein Klick auf Sequenz 2 ergibt `Store Sequence 2`
und schickt es ab, weil der Zeiger das fehlende Argument geliefert hat. Steht
nichts in der Zeile, tut derselbe Klick das, was die Kachel immer tut.

Drei Fälle, in denen nichts abgeschickt wird, und alle drei aus demselben Grund
— dass Getipptes nicht verschwinden darf:

- **Die Zeile ist keine Verb-Zeile.** `1 thru` plus ein Klick auf eine Gruppe ist
  ein Bereich im Bau, keine Gruppe, die benannt wird.
- **Das Verb ist `Label` oder `Color`.** Beide sind ohne letztes Wort gültig und
  **nehmen etwas weg**. Ein Klick, der einen Namen löscht, wäre die schlimmste
  Abkürzung, die es gibt.
- **Es ist noch kein vollständiger Befehl.** `Store Fixture 5` bleibt stehen, mit
  der Erklärung des Pults darunter — statt dass das getippte `Store` still
  verschwindet und ein Fixture ausgewählt wird.

### Was **keine** Zeile ist

Absichtlich, und vollständig: die Executor-Tasten und -Fader (ein `Go` ist eine
Geste mit Zeit darin, ein Fader ein Strom von Positionen); die Encoder, die fünf
Bank-Tasten und die zwei Parameterpfeile; Fenster ziehen und in der Größe
ändern; **Add window**, weil es einen Fenster*typ* benennt und keine Nummer; und
die drei Zeiten und der Trigger einer Cue sowie die Felder im Patch-Formular —
die tragen einen Wert, statt einen Ort zu benennen.

---

## 11. Das X-Touch

Ein Behringer X-Touch über USB, im **MC-Modus**. Es ist ein vollwertiges
Bedienpult für dieses Programm und nicht eine Fernbedienung dafür: es
funktioniert, **ohne dass irgendein Fenster offen ist**.

Angeschlossen wird es in *Settings → Devices*: der MIDI-Port wird über seinen
**Namen** ausgewählt, weil das das Einzige ist, was ein Aus- und Einstecken
übersteht. Ein Pult, das nicht eingesteckt ist, ist eine Warnung und kein
Startproblem; eines, das später eingesteckt wird, wird ohne Neustart
aufgenommen.

Was die Tasten tun, steht in *Settings → Controls* und ist **jede einzelne
umbelegbar**, mit einem *Learn*: Editor auf, Taste drücken, sie wird benannt und
nicht ausgelöst. Eine Taste kann auch eine ganze Zeile schicken — dieselbe
Möglichkeit wie beim Executor in Kapitel 8, und derselbe Weg.

**Was von der Hardware bekannt und unangenehm ist:** sättigt man beide
Richtungen gleichzeitig, kann das X-Touch aufhören zu **senden**, während es
weiter empfängt, und nur ein Aus- und Einschalten holt es zurück. Das ist eine
Eigenschaft des Geräts, gegen die dieses Pult ausdrücklich designt ist (es
sendet Rückmeldung als Differenz und nicht als Neuzeichnung), und es ist in
[`../MCU_MAPPING.md`](../MCU_MAPPING.md) §2.7 dokumentiert.

Jede Notennummer, jede CC-Nummer und jede Kennzeichnung ist an einem echten
X-Touch (MC-Modus, USB, Firmware V1.25) Bedienelement für Bedienelement geprüft
worden. Die Aufzeichnungen liegen im Repository und laufen in der normalen
Testsuite mit, ohne dass etwas angeschlossen sein muss.

---

## 12. Show-Dateien, Sicherung und Umzug

**Speichern:** `Ctrl-S`, oder *Settings → Show files*. Dort auch *Save as*,
*Open* und *New*, jeweils mit dem **Dateidialog des Betriebssystems**.

**Eine `.prism`-Datei ist in sich vollständig.** Die Fixture-Profile, mit denen
Sie gepatcht haben, liegen darin. Eine Show auf einem Stick in ein anderes Haus
zu tragen funktioniert deshalb — und was **nicht** mitgeht, ist die Verkabelung:
die Ausgänge gehören dem Gebäude und stehen in `machine.json` neben der Show,
nicht darin. Das ist Absicht, und das andere Handbuch erklärt sie.

**Die Wiederherstellungskopie.** Solange etwas ungespeichert ist, schreibt das
Pult alle dreißig Sekunden eine Kopie neben die Show. Das **gilt nicht als
gespeichert** — die Save-Anzeige bleibt an, bis Sie wirklich speichern.

**Wo die Daten liegen** (Windows):

| | Wo |
|---|---|
| Das Programm | `%LOCALAPPDATA%\PrismDMX` |
| Shows, Einstellungen, Ihre Profile, das Protokoll | `%APPDATA%\PrismDMX` |

**Deinstallieren entfernt das Programm und sonst nichts.** Ihre Shows,
`machine.json`, Ihre Fixture-Korrekturen und Ihre Tastenbelegung bleiben liegen,
und eine spätere Installation findet sie wieder.

---

## 13. Was dieses Pult noch nicht kann

Hier genannt, statt von Ihnen entdeckt zu werden:

- **Kein 3D-Visualizer.** Geplant.
- **Keine Web-Fernbedienung** — ein Telefon oder Tablet kann das Pult noch nicht
  fahren. Geplant.
- **Kein Timecode, kein OSC, kein PSN.** Geplant.
- **Keine Effekt-Engine** — das Fenster *Phaser Editor* ist leer, weil das
  darunter fehlt.
- **Das Pult folgt einem Moduskanal nicht**, während die Show läuft (Kapitel 6,
  `B52`).
- **Autostart nur unter Windows.** Die Einstellung gibt es überall, der Eintrag
  wird nur unter Windows geschrieben.
- **macOS und Linux werden nicht veröffentlicht.** Die Engine ist portabel und
  wird bei jedem Commit für ARM64 gebaut, aber nur für Windows gibt es ein
  Installationsprogramm.

### Wenn Sie etwas finden

Das Nützlichste ist keine Fehlerliste, sondern **was passiert ist, als Sie etwas
Echtes versuchen wollten**. Ein Eintrag, der sich als beabsichtigt herausstellt,
kostet eine Zeile Erklärung; ein Fehler, den niemand aufgeschrieben hat, kostet
den ersten Abend, an dem jemand anders damit eine Show fährt.

Melden unter
[github.com/flakesystems/PrismDMX/issues](https://github.com/flakesystems/PrismDMX/issues),
mit: was Sie getan haben (die Schritte, der Reihe nach), was passiert ist, was
Sie erwartet hatten, **die Version** (*Settings → This machine*) und **das Rig** —
und ob es auch passiert, wenn nichts angeschlossen ist.

Dazu, wenn möglich: das Protokoll aus `%APPDATA%\PrismDMX` (die Stufe lässt sich
in *Settings → This machine* hochdrehen), die Show-Datei, und `machine.json`,
wenn es um Ausgänge oder das Bedienpult geht — **lesen Sie die vorher**, sie
enthält das Zugriffstoken dieses Pults, falls Sie eines gesetzt haben.
