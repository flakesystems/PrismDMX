# Änderungen

Was sich für jemanden geändert hat, der das Programm benutzt — eine Liste aller
Versionen, kurz.

**Auf Deutsch, und `docs/RELEASE_NOTES.md` auf Englisch.** Das sind nicht zwei
Kopien desselben Textes: hier steht **jede** Version in ein paar Zeilen, dort
steht die **aktuelle** in voller Länge, und dieser Text ist es, der auf der
GitHub-Release-Seite neben dem englischen `README.md` erscheint. Ein Test hält
die beiden zusammen — eine Version, die dort steht und hier nicht, macht den
Build rot.

**Er wird geschrieben, nicht erzeugt.** `PROGRESS.md` §2 wäre die naheliegende
Quelle und ist die falsche: das ist ein Verifikationsprotokoll für Entwickler,
in dem Sätze über Tests und Messungen stehen. Daraus erzeugte Prosa wäre genau
das Dokument, mit dem ein Benutzer nichts anfangen kann. Mechanisch ist die
**Menge der Versionen**, und die wird geprüft.

Alle Versionen bisher sind **Vorabversionen**.

---

## Noch nicht veröffentlicht

**Diese Änderungen erscheinen als 0.9.3.** Die Version steht noch nicht auf dem
Programm — der Eigentümer hielt das Release zurück, bis die Controls-Änderungen
(S59) und der 3D-Viewer (S30) mit drin sind, und beide sind es jetzt.

### Der 3D-Viewer (S30)

**Das Fenster *Viewer 3D* zeigt das Rig, wie es hängt**, und aus jedem Fixture,
das leuchtet, einen Strahl in der Farbe, die es gerade ausgibt. Die Strahlen
kommen **vom Kabel** — dieselben Werte wie im DMX Sheet —, also sieht man, was
dem Rig tatsächlich gesagt wird: Pan, Tilt, Zoom, Dimmer, Farbe, ein
geschlossener Shutter.

- **Fixtures platzieren.** Ein Feld neben dem Bild arbeitet mit der Auswahl:
  *Set* setzt Position und Rotation, *Spread* verteilt die Auswahl quer über die
  Bühne — eine Traverse mit acht Geräten in einem Zug. Jedes davon ist **ein
  Oops**, und das Platzieren kostet die DMX-Ausgabe nichts.
- **Umsehen** mit der Maus, dazu *Front*, *Top*, *Side*, *3D* und *Frame all*.
  Die Kamera gehört dem Bildschirm; ein zweiter Bildschirm darf das Rig von
  woanders zeigen.
- **Ein Klick auf ein Fixture** wählt es aus wie ein Klick im Fixture Sheet.
- Ein Profil aus einer **GDTF**-Datei wird in seiner Größe gezeichnet, mit dem
  Strahl dort, wo der Hersteller ihn angibt; jedes andere als kleiner Kasten.
  Die eigenen 3D-Modelle der Geräte und die Gobo-Bilder kommen später.
- Gezeichnet wird auf einer gewöhnlichen 2D-Fläche, damit es auf jedem Rechner
  läuft, auf dem das Pult läuft — auch ohne Grafikkarte.

### Die Fixture-Bibliothek ist jetzt GDTF (S61)

**Das Pult liest [GDTF](https://gdtf.eu)** — das Format, in dem Hersteller ihre
Geräte veröffentlichen und in dem ein Rig zwischen Programmen ausgetauscht wird.
Eine `.gdtf`-Datei bringt mit, was eine Kanalliste nicht kann: **die Bilder der
Gobos**, das **3D-Modell** des Geräts, seine Maße, und **wo der Strahl
austritt** und wohin er zeigt. Das ist es, was der 3D-Viewer braucht, und darum
kommt es vor ihm.

- **Die installierte Bibliothek ist GDTF.**
  `tools/fetch-fixtures/fetch-fixtures` installiert sie, aus einem Ordner voller
  `.gdtf`-Dateien oder aus einem kostenlosen Konto bei
  [gdtf-share.com](https://gdtf-share.com) — dieser Dienst hat keinen anonymen
  Massen-Download, darum fragt das Skript danach und sagt es, wenn man ihm
  nichts gibt.
- **Eigene Fixtures im Format der Open Fixture Library laufen weiter.** Eine
  Lampe, für die niemand eine GDTF veröffentlicht hat, schreibt man weiterhin
  als JSON in `fixtures/` im Datenverzeichnis; das ist von Hand weit leichter zu
  schreiben als ein ZIP voller XML. Beides steht nebeneinander in der Liste, und
  was im Datenverzeichnis liegt, gewinnt weiterhin.
- **Eine `.gdtf`-Datei im eigenen Ordner ersetzt das Fixture in der Bibliothek,
  egal wie sie heißt** — der Schlüssel kommt aus der Datei und nicht aus dem
  Dateinamen.
- **Das Patch-Fenster sagt, woher ein Profil kommt.** Eine neue Spalte *Format*
  (`GDTF` oder `OFL`), und unter dem Namen des gewählten Fixtures eine Zeile, die
  sagt, was es mitbringt — *3D model · 1 beam*.
- **Gobos haben Namen und Bilder.** Wo eine GDTF ein Rad beschreibt, heißen die
  Stufen eines Kanals wie die Slots des Rades, und der Name des Bildes reist mit
  dem Profil in die Show.
- Das Pult sagt beim Start, wie viele Profile es anbietet und **wie viele davon
  GDTF sind** — und sagt es eigens, wenn eine Bibliothek installiert ist, die
  noch keine GDTF enthält.

### Vier Wege, die Bibliothek zu füllen (S62)

**Eine GDTF muss man jetzt nicht mehr von Hand in einen Ordner legen.** Das
Patch-Fenster hat zwei neue Knöpfe, und die Einstellungen einen Abschnitt:

- ***Import profile (GDTF)*** nimmt eine einzelne `.gdtf` über den Dateidialog,
  liest sie, legt sie in Ihren Fixture-Ordner und bietet sie sofort im
  Auswahlfeld an. Was keine lesbare Fixture-Datei ist, landet gar nicht erst im
  Ordner.
- ***Import rig (MVR)*** nimmt die Rig-Datei Ihres Planers **in die Show**: jedes
  Profil darin und jedes geplante Fixture mit Nummer und Adresse, **in einem
  Schritt, den ein Oops ganz zurücknimmt**. Gepatchtes wird nicht angefasst —
  ein geplantes Fixture, dessen Nummer Ihre Show schon benutzt, bekommt die
  nächste freie. Danach sagt das Pult, wie viele ankamen und was es übersprungen
  hat. Eine `.mvr`, die einfach im Fixture-Ordner liegt, füllt auch ohne den
  Knopf die Bibliothek.
- ***Settings → This machine → GDTF Share*** lädt die ganze veröffentlichte
  Bibliothek **mit Ihrem eigenen Konto** bei [gdtf-share.com](https://gdtf-share.com)
  in Ihren Fixture-Ordner. Eine Zeile zählt mit, das Pult spielt dabei weiter
  Licht, und am Ende steht ein Satz. Auf Wunsch merkt sich Windows Ihre
  Zugangsdaten in der **Anmeldeinformationsverwaltung** — nie in einer
  Einstellungsdatei; *Forget this account* nimmt sie wieder heraus.

**Ein Konto braucht niemand.** Die Open Fixture Library ist weiter dabei, eine
eigene Datei im Ordner gewinnt weiter gegen die installierte, und eine `.mvr`
löst den realen Fall ganz ohne Internet. Warum das Pult die GDTF-Bibliothek
nicht einfach mitliefert, steht in `docs/FIXTURE_LIBRARY.md`.

Was die offene Beta in den ersten zwei Wochen gemeldet hat — zehn Meldungen,
alle zehn behoben (S56 und S57), dazu der letzte offene Eintrag des Registers
(B52, S58). **Alle zehn GitHub-Issues sind geschlossen** (#9, #21, #23–#30):

- **Controls:** eine Taste auf *Fenster wählen* oder *Befehl schreiben* zu
  binden trennte den Client bei jedem Öffnen des Menüs. Behoben (B55).
- **`fixtures/`** wird beim ersten Start angelegt (B54).
- **Zahlenfelder** im Patch- und im Output-Formular lassen sich beim Tippen
  leeren; geprüft wird beim Anwenden (B53).
- **Kommandozeile:** ein View-Wechsel lässt die Zeile stehen (B56); Oops nimmt
  zuerst das letzte Wort weg, auch am X-Touch (B58); die Rückmeldung verschiebt
  nichts mehr (B61).
- **Fenster:** ein Klick in ein nicht fokussiertes Fenster wählt sofort aus
  (B57).
- **Crossfade:** alle Griffe zeigen dieselbe Stellung, und der Motorfader fährt
  nach dem Loslassen nicht mehr zurück (B59).
- **Cue Viewer:** keine Store-Leiste mehr — Cues werden auf der Kommandozeile
  gespeichert; die Update-Taste blinkt im Fenster *CommandKeys* (B62).
- **Patch-Fenster**, neu gebaut um die Bibliothek (B60, S57): *Add fixture*
  öffnet die Bibliothek und die Einstellungen nebeneinander; jedes Fixture ist
  **eine** Zeile, der Modus wird daneben gewählt, und die ganze Zeile wählt; die
  Liste lädt beim Scrollen nach; ein neues Fixture beginnt an der **nächsten
  freien Adresse**, die eine Überlappung auch nennt; ein Fixture ohne Namen heißt
  wie sein Typ; und **mehrere desselben Typs** werden in einem Schritt gepatcht,
  den ein Oops ganz zurücknimmt.
- **Moduskanäle** (B52): ein Knopf, dessen Kanal ein anderer umschaltet — der
  Nachbar von *Mode Select* an einer ADJ Flat Par QA12 —, heißt jetzt, was er
  gerade ist (*Strobe*, *Program Speed*, *Sound Sensitivity*), und bietet dessen
  Stufen an. Es zählt, was am Kabel anliegt, also folgt er auch einer Cue.

**Und die Controls-Runde (S59)** — das Pult bekommt das Vokabular der
Kommandozeile, und seine Lampen fangen an, etwas zu sagen:

- **Jede Konsolentaste lässt sich auf das Pult legen.** `Store`, `Edit`,
  `Label`, `Color`, `New`, `Fixture`, `Cue`, `At`, `Thru` und die übrigen sind
  dieselbe Liste, die das Fenster *Command Keys* zeigt — eine Tabelle für beide
  Geräte, damit kein Wort nur auf einem davon ankommt. `Color`, `New`, `At` und
  `Thru` sind neu und auch am Bildschirm dazugekommen.
- **Eine belegte Taste leuchtet, wenn ein Druck etwas bewürde.** Bisher gab es
  auf dem ganzen Pult zwei Lampen, und beide hängen am Platz der Taste. Jetzt
  hängt die Lampe an dem, was auf der Taste **liegt**: `Store` bleibt dunkel, bis
  der Programmer etwas hält, `Clear` leuchtet, bis das dreistufige Clear nichts
  mehr wegzunehmen hat, ein Argumentwort wie `Cue` leuchtet erst, wenn ein Verb
  auf sein Objekt wartet.
- **Das Jog-Rad ist deutlich schneller**, und zwar in der Einheit, die
  entscheidet, ob man etwas sieht: eine langsame Raste bewegt jetzt **genau
  einen DMX-Schritt** statt einem Dreizehntel davon, eine schnelle vier. Dazu
  ein Regler in *This machine → Jog wheel*, 10 bis 400 %.
- **Die Controls-Seite ist aufgeräumt.** Die Strips, der Main-Fader und das
  Jog-Rad gehören dem Executor-Fenster und dem Programmer und sind in einen
  ausklappbaren Bereich *Advanced* gewandert; die Spalte *Strip / Selected* ist
  weg, weil die verbliebenen Zeilen alle den selektierten Executor meinen.
- **Die Controls-Seite hat ein Bild des Pults.** Neben der Liste eine
  maßstäbliche Zeichnung der Oberfläche mit jeder Taste an ihrem Platz — gefüllt
  heißt belegt, hohl heißt frei, und was am echten Pult leuchtet, leuchtet hier
  mit. So sieht man beim Einrichten, welche Tasten noch frei sind.
- **Die mitgelieferte Tastenbelegung ist vollständig** — jede Taste des Panels
  außer SMPTE/Beats (reserviert) und Name/Value (ohne Lampe) tut etwas. Beim
  Update wird eine eigene Tabelle **ersetzt**; vorher exportieren, wer sie
  behalten will.

---

## 0.9.2 — die Fixture-Bibliothek, vollständig

*6. September 2026 ·
[Release](https://github.com/flakesystems/PrismDMX/releases/tag/v0.9.2)*

Alles hier fing damit an, dass der Eigentümer etwas mit `0.9.1` tun wollte und
nicht konnte. Es ist ein Thema: **was aus einem Fixture-Profil wird, wenn man es
patcht.**

- **Ein Fixture mit zwei Kanälen derselben Art behält beide.** Ein Kopf mit zwei
  Farbrädern, eine LED-Röhre mit einem Rot je Pixel: über die installierte
  Bibliothek waren das **2 679 Kanäle**, die es schlicht nicht gab. Sie sind
  jetzt da und **durchnummeriert** — *Gobo* und *Gobo 2*, `1 gobo 2 at 50`. Wo
  mehr Wiederholungen sind, als nebeneinander passen, bekommt das
  Programmer-Band einen **Part**-Schalter.
- **Warmweiß und Kaltweiß sind zwei Lampen.** Vorher waren sie ein Attribut, und
  auf einem Fixture mit beiden antwortete der zweite Kanal gar nicht. Das Pult
  kennt jetzt **41 Parameter** statt 34.
- **Die Encoder-Bänke zeigen nur, was die ausgewählten Fixtures haben.** Ein PAR
  ausgewählt heißt vier Farbknöpfe, nicht dreizehn.
- **Die Namen der Radstellungen sind die des Herstellers**, und ein
  **Rechtsklick** auf den Encoder öffnet die Liste. Vorher las jede vierte
  Stellung *Slot 3*.
- **Profile mit Pixel-Matrix lassen sich patchen.** 90 der 634 Fixtures
  beschreiben ihre Kanäle als *einmal je Pixel wiederholen*, und jeder solche
  Modus wurde übersprungen. Die Bibliothek liefert jetzt **2 871 Profile** statt
  2 157.
- **Jeder DMX-Kanal eines gepatchten Fixtures hat einen Knopf**, ohne Ausnahme.
  Wo das Profil nicht sagt, was ein Kanal tut, steht er auf der Bank *Control*
  unter dem Namen des Herstellers oder als `Ch 7`; von der Zeile aus
  `1 raw 3 at 50`. Vorher waren **707 Kanäle** von 337 Profilen gar nicht
  erreichbar.
- **Jeder Encoder trägt den Namen, den der Hersteller dem Kanal gegeben hat**,
  und ein Rad steht auf der Bank, die sein *Inhalt* sagt — ein Farbrad unter
  *Colour*, auch wenn das Profil es anders nennt.

Eine `.prism`-Datei aus `0.9.1` öffnet unverändert.

## 0.9.1 — die zweite geschlossene Beta

*6. September 2026 ·
[Release](https://github.com/flakesystems/PrismDMX/releases/tag/v0.9.1)*

Die erste Version, die aus dem gebaut wurde, was die erste Beta gefunden hat.
Sechs Einträge, vier davon Fehler.

- **Der Crossfade-Fader funktioniert wie ein Crossfade-Fader.** Vorher sprang er
  nach jeder Blende auf null zurück. Jetzt **bewegt ihn nie etwas**, und es gibt
  zwei Modi je Executor: **XFade** blendet hoch auf die nächste Cue und runter
  auf die übernächste, **Fade** blendet die laufende aus und die nächste ein.
  Auf halbem Weg stehen bleiben hält die Mischung.
- **`Clear` lässt los, bevor es vergisst.** Erster Druck: die **Auswahl** fällt
  weg, der Look bleibt. Zweiter: die Werte. Dritter: Bank und Seite. Das ist die
  Reihenfolge, in der Menschen einen Look aus mehreren Fixtures bauen.
- **Ein Drittel der Fixture-Bibliothek kam mit fehlenden Kanälen an.** 5 037 von
  15 150 Kanälen erreichten gar nichts — alle CMY-Farbmischung, jedes Farbrad,
  jeder eingebaute Effekt, Frost, Nebel, die Blenden. Das Pult kennt jetzt **34
  Parameter** statt 15, und **kein Kanal der Bibliothek bleibt unzugeordnet**.
- **Kanäle mit benannten Stellungen sagen das.** Der Encoder nennt die Stellung,
  in der er steht, und bietet die Liste an.
- **Eigene Fixture-Profile haben einen Ort:** `fixtures/` im Datenverzeichnis,
  das keine Installation anrührt.
- **`F11` und `Alt` + `Enter` schalten Vollbild.**
- **Ein aus dem Task-Manager beendetes Pult hinterlässt kein totes Symbol mehr**
  im Infobereich.

## 0.9.0 — die erste geschlossene Beta

*1. September 2026 ·
[Release](https://github.com/flakesystems/PrismDMX/releases/tag/v0.9.0)*

Die erste Version, die jemand installieren konnte: aus zwei Prozessen wurde ein
Programm.

- **Ein Installationsprogramm**, pro Benutzer und ohne Administratorrechte, das
  die Engine, die Oberfläche und die Fixture-Bibliothek zusammen mitbringt — und
  das aus der CI kommt und nicht von einem Rechner.
- **Ein Fenster, das man schließen kann, ohne die Show zu beenden**, mit einem
  Symbol im Infobereich, das das sagt, und einem geordneten Anhalten.
- **Ein zweiter Start hängt sich an das laufende Pult an**, statt ein zweites zu
  starten.
- **Die Dateidialoge des Betriebssystems** für jeden Pfad.
- **Autostart** als Kästchen, ohne Administratorrechte, mit einer Zeile, die
  sagt, was die Maschine wirklich hat.

Darunter lag das, was in den Sessions davor entstanden ist: das Pult als eigener
Prozess, der Merge und der 44-Hz-Tick, Art-Net, sACN und Open DMX USB, der
Programmer, Cue-Listen mit Tracking, Executors, das X-Touch, die Kommandozeile
als Bedienung und das Einstellungsfenster.
