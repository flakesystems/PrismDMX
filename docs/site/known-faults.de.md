# Bekannte Fehler

**Gilt für 0.9.2.** Was in der veröffentlichten Version kaputt ist, hier
aufgeschrieben, damit Sie es finden, bevor es Sie mitten in einer Vorstellung
findet.

Diese Seite zeigt **nur noch offene Fehler**. Was behoben ist, steht in den
[Änderungen](/de/aenderungen/), und das vollständige Register — jeder je
gemeldete Eintrag, was daraus wurde und welcher Test ihn heute festhält — liegt
im Repository, weil es ein Arbeitsdokument ist und kein öffentliches.

Zum Zeitpunkt dieser Fassung sind in 0.9.2 **elf** Fehler offen, von
einundsechzig gemeldeten — **neun** davon sind für die nächste Version schon
behoben und hier so markiert.

## B52 — Das Pult folgt einem Switching Channel nicht, während die Show läuft

**Wo:** Fixture-Bibliothek und Encoder-Band im Programmer.
**Schwere:** kosmetisch.

**Was passiert.** Manche Fixtures benutzen einen Kanal, um die Bedeutung der
*anderen* Kanäle umzuschalten — einen Moduskanal, in der Sprache der Open Fixture
Library ein *Switching Channel*. Seit S54 ist der betroffene Slot immer
erreichbar, und überall dort, wo sich alle Stellungen einig sind, was der Slot
tut, trägt er auch den richtigen Namen. Drehen Sie den Moduskanal aber im
laufenden Betrieb, ändert sich die Bedeutung des Nachbarslots — und seine
Beschriftung folgt nicht. Der Knopf heißt weiter, wie er hieß.

**So sehen Sie es.** Patchen Sie ein Fixture mit Moduskanal — meistens ein Laser
— und drehen Sie den Modus. Der Nachbar-Encoder heißt immer noch `Channel 2`,
oder wie auch immer er vorher hieß.

**Was es nicht tut.** Es sendet keinen falschen Wert: der Kanal wird weiterhin
richtig adressiert, und `1 raw 3 at 50` fährt nach wie vor den dritten solchen
Kanal von Fixture 1. Veraltet ist die *Beschriftung*, nicht der Ausgang.

**Warum es noch offen ist.** Das ist keine fehlende Zeile im Bibliotheks-Reader.
Ein Attribut, dessen Bedeutung vom Wert eines anderen Kanals abhängt, ist etwas,
wofür das Datenmodell keinen Begriff hat — und der Schlüssel, unter dem eine Cue
einen Wert ablegt, darf sich nicht ändern, während diese Cue läuft, sonst
bedeutete die Cue beim Abfahren etwas anderes als beim Speichern. Die Behebung
ist also eine Frage an das Modell und nicht die Arbeit eines Nachmittags, und sie
wird auch so behandelt.

## B53 — Pflichtfelder lassen das vollständige Leeren während der Eingabe nicht zu

**Behoben für die nächste Version.** Ein Zahlenfeld lässt sich beim Tippen leeren; geprüft wird erst beim Anwenden.

**Wo:** UI, Pflichtfelder (Required Inputs).
**Schwere:** ärgerlich.

**Was passiert.** Felder, die als Pflichtfelder markiert sind, lassen das
vollständige Leeren während der Eingabe nicht zu — das verhindert, die erste
Stelle einer Zahl oder den ersten Buchstaben eines Worts zu ändern.

**So sehen Sie es.** Ein Pflichtfeld (z.B. Adress- oder Namensfeld) auswählen,
Inhalt komplett löschen versuchen — das Feld lehnt ab.

## B54 — Das `fixtures/`-Verzeichnis wird bei der Installation nicht angelegt

**Behoben für die nächste Version.** Das Pult legt `fixtures/` beim ersten Start an, mit einer `README.txt` darin.

**Wo:** Installation, Datenverzeichnis des Daemons.
**Schwere:** ärgerlich.

**Was passiert.** `fixtures/` existiert nach einer Neuinstallation nicht. Wer
ein eigenes Profil ablegen möchte, muss das Verzeichnis von Hand anlegen — ohne
Hinweis darauf, dass es fehlt.

**So sehen Sie es.** PrismDMX neu installieren, Datenverzeichnis öffnen —
`fixtures/` fehlt.

## B55 — Das Controls-Menü bricht zusammen, wenn eine neue Taste gebunden wird

**Behoben für die nächste Version.** Eine Taste zu binden trennt den Client nicht mehr — gleich welche Aktion.

**Wo:** Settings, Controls-Menü.
**Schwere:** blocker.

**Was passiert.** Bindet man eine neue Taste im Controls-Menü, trennt sich der
Client und verbindet sich neu — zurückgesprungen auf das Output-Menü. Dieser
Absturz tritt danach bei jedem Öffnen des Controls-Menüs erneut auf, auch nach
einem Daemon-Neustart.

**Abhilfe.** Den gebundenen Control manuell aus `machine.json` entfernen und den
Daemon neu starten.

**So sehen Sie es.** Settings öffnen → Controls → eine neue Taste binden.

## B56 — Ein Ansichtswechsel löscht die Kommandozeile

**Behoben für die nächste Version.** Ein View-Wechsel lässt eine halb getippte Zeile stehen.

**Wo:** Canvas/Views, Kommandozeile.
**Schwere:** ärgerlich.

**Was passiert.** Wenn man während einer laufenden Eingabe die Ansicht wechselt,
wird die Kommandozeile geleert — da Ansichtswechsel intern als Kommando
ausgeführt werden.

**So sehen Sie es.** Etwas in die Kommandozeile tippen, dann die Ansicht
wechseln — die Zeile ist leer.

## B57 — Der Fensterfokus verhindert die Auswahl beim ersten Klick

**Behoben für die nächste Version.** Ein Klick wählt aus, auch in einem Fenster, das nicht fokussiert war.

**Wo:** Canvas, alle Fenster.
**Schwere:** ärgerlich.

**Was passiert.** Wenn ein anderes Fenster den Fokus hat, fokussiert der erste
Klick auf ein Element in einem anderen Fenster nur das Fenster — die eigentliche
Auswahl findet erst beim zweiten Klick statt.

**So sehen Sie es.** Ein anderes Fenster fokussieren, dann auf ein Fixture im
Fixture Sheet klicken — erster Klick fokussiert nur, zweiter wählt aus.

## B58 — Oops löscht keine Wörter aus der Kommandozeile

**Behoben für die nächste Version.** Oops nimmt zuerst das letzte Wort der Zeile weg, auch am X-Touch.

**Wo:** Kommandozeile, Oops-Taste.
**Schwere:** ärgerlich.

**Was passiert.** Wenn etwas in der Kommandozeile steht, wirkt Oops sofort als
Undo, ohne zuerst die Eingabe zu löschen. An Pulten ohne Tastatur gibt es so
keinen Weg, Tippfehler in der Zeile zu korrigieren.

**So sehen Sie es.** Etwas in die Kommandozeile tippen und Oops drücken — die
Eingabe bleibt, eine Aktion wird rückgängig gemacht.

## B59 — Crossfade-Fortschritt wird nicht zwischen Clients synchronisiert

**Behoben für die nächste Version.** Alle Griffe auf einen Crossfade zeigen dieselbe Stellung, der Motorfader eingeschlossen.

**Wo:** Executor Strip, Crossfade.
**Schwere:** ärgerlich.

**Was passiert.** Der Fortschritt eines Crossfade-Executors wird nicht zwischen
Web-Client und MIDI-Client synchronisiert. Die Ausgabe ist korrekt, aber der
Fader springt auf die Position des Web-Clients zurück, sobald man den
MIDI-Fader loslässt.

**So sehen Sie es.** Einen Crossfade-Executor anlegen, mit dem X-Touch-Fader
bewegen, während ein Web-Client verbunden ist.

## B60 — Patch-Fenster: mehrere Fehler und Verbesserungen

**Wird als eigene Session umgebaut (S57)**, weil die acht Punkte aneinanderhängen. Bis dahin lassen sich die Zahlenfelder immerhin leeren und neu tippen.

**Wo:** Patch-Fenster, Fixture-Bibliothek.
**Schwere:** ärgerlich.

**Was passiert.** Mehrere bekannte Probleme im Patch-Fenster: Fixtures desselben
Typs mit verschiedenen Modi erscheinen als separate Einträge; nur die erste Spalte
einer Bibliothekszeile ist klickbar; beim Scrollen werden keine weiteren Fixtures
nachgeladen; der „Fixture hinzufügen"-Knopf öffnet nicht direkt die Bibliothek;
die Fehlermeldung bei überlappenden Adressen nennt nicht die nächste freie
Adresse; neue Fixtures starten nicht mit der nächsten freien Adresse; Fixtures
ohne Namen bekommen nicht automatisch ihren Typ als Namen; mehrere Fixtures
desselben Typs lassen sich nicht gleichzeitig patchen.

## B61 — Das Befehlsfeedback verschiebt das Layout

**Behoben für die nächste Version.** Die Rückmeldung der Kommandozeile steht in einer Zeile fester Höhe.

**Wo:** Kommandozeile, Befehlsfeedback.
**Schwere:** ärgerlich.

**Was passiert.** Wenn man etwas in die Kommandozeile tippt, erscheint ein
Befehlsfeedback, das Teile des UI verschiebt.

**So sehen Sie es.** Etwas in die Kommandozeile tippen und beobachten, wie das
UI springt.

## B62 — Store-Leiste im Cue Viewer

**Behoben für die nächste Version.** Die Store-Leiste ist weg; die Update-Taste blinkt im Fenster *CommandKeys*.

**Wo:** Cue Viewer.
**Schwere:** kosmetisch.

**Was passiert.** Am unteren Rand des Cue Viewer gibt es eine Sektion, über die
der aktuelle Programmer-Inhalt in einen Cue gespeichert oder ein neuer Cue
erstellt werden kann. Diese Sektion ist überflüssig — Cues werden über die
Kommandozeile gespeichert.

## Etwas anderes stimmt nicht

Wenn Sie auf etwas stoßen, das hier nicht steht, ist das eine Meldung wert —
auch wenn Sie nicht sicher sind, ob es ein Fehler ist. Besonders dann, wenn Sie
beim Befolgen der Dokumentation hängen geblieben sind: das ist ein Fehler in der
Dokumentation und keiner bei Ihnen.

[Einen Fehler auf GitHub melden](https://github.com/flakesystems/PrismDMX/issues).
Am meisten hilft: was Sie getan haben, was Sie erwartet haben, was stattdessen
passiert ist, und ob es jedes Mal passiert.
