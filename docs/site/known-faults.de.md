# Bekannte Fehler

**Gilt für 0.9.2.** Was in der veröffentlichten Version kaputt ist, hier
aufgeschrieben, damit Sie es finden, bevor es Sie mitten in einer Vorstellung
findet.

Diese Seite zeigt **nur noch offene Fehler**. Was behoben ist, steht in den
[Änderungen](/de/aenderungen/), und das vollständige Register — jeder je
gemeldete Eintrag, was daraus wurde und welcher Test ihn heute festhält — liegt
im Repository, weil es ein Arbeitsdokument ist und kein öffentliches.

Zum Zeitpunkt dieser Fassung ist **ein** Fehler offen, von einundfünfzig
gemeldeten.

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

## Etwas anderes stimmt nicht

Wenn Sie auf etwas stoßen, das hier nicht steht, ist das eine Meldung wert —
auch wenn Sie nicht sicher sind, ob es ein Fehler ist. Besonders dann, wenn Sie
beim Befolgen der Dokumentation hängen geblieben sind: das ist ein Fehler in der
Dokumentation und keiner bei Ihnen.

[Einen Fehler auf GitHub melden](https://github.com/flakesystems/PrismDMX/issues).
Am meisten hilft: was Sie getan haben, was Sie erwartet haben, was stattdessen
passiert ist, und ob es jedes Mal passiert.
