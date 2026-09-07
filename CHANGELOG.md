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
