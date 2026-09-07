# Die Kommandozeile

**Gilt für 0.9.2.**

Die Kommandozeile ist keine Abkürzung für Leute, die die Maus nicht mögen. Sie
ist die eigentliche Bedienung des Pults: jede Taste auf der Oberfläche schreibt
eine Zeile, und jede Zeile lässt sich auf eine Taste legen. Was Sie sagen können,
können Sie binden — und was Sie mit einem Knopf tun, können Sie nachlesen.

Eine Zeile liest sich von links nach rechts und tut nichts, bis Sie `Enter`
drücken. Bis dahin können Sie sie ändern, und `Escape` wirft sie weg.

## Die Form einer Zeile

```
1 thru 8 at 50            Fixtures 1 bis 8 auf die Hälfte
1 + 5 + 9 at full         drei Fixtures, ganz hoch
Group 3 at 25             alles in Gruppe 3
5 pan at 25               ein Parameter eines Fixtures
1 at out                  und wieder auf null
```

**Eine nackte Zahl ist ein Fixture.** `1 at full` und `Fixture 1 at full` sind
dieselbe Zeile; das Hauptwort steht für alle da, die es aus Gewohnheit von einem
anderen Pult tippen.

**`thru` ist ein Bereich, `+` fügt hinzu, `-` nimmt weg.** Das lässt sich
kombinieren: `1 thru 8 - 5` sind sieben Fixtures.

## Die Wörter

Neunundzwanzig, und diese Tabelle wird von einem Test gegen die Liste des Pults
selbst gehalten — ein Wort, das hier steht, gibt es dort also auch.

| Wort | Was es tut |
|---|---|
| `assign` | Legt eine Cue-Liste auf einen Executor (`Assign Sequence 5 Executor 1`) oder sagt, was ein Fader, ein Encoder oder eine der vier Tasten tut (`Assign Executor 1 Fader XFade`). Welche der beiden Bedeutungen gilt, entscheidet das erste Hauptwort |
| `at` | Setzt einen Wert: `at 50`, `at full`, `at out`, `5 pan at 25` |
| `clear` | Die drei Stufen: erst die Auswahl, dann die Werte, dann Bank und Seite |
| `color` | Gibt einem Objekt eine Farbe für den Scribble-Strip: `Color Sequence 4 blue`. Ohne letztes Wort **nimmt es die Farbe weg** |
| `copy` | Kopiert ein Objekt auf eine andere Nummer: `Copy Sequence 2 Sequence 6`. Ebenso für Cues, Gruppen, Presets und Views |
| `cue` | Das Hauptwort für eine Cue. Allein ist es **keine Zeile** — `Cue 5` könnte ein Goto oder ein Edit sein, und das Pult sagt das, statt zu raten |
| `delete` | Löscht ein Objekt: `Delete Cue 3` |
| `edit` | Lädt eine Cue in den Programmer, um sie zu ändern: `Edit Cue 3`, `Edit Sequence 5 Cue 3` |
| `executor` | Das Hauptwort für einen Executor. Allein — `Executor 3` — wählt den, auf den der Transport wirkt |
| `fixture` | Das Hauptwort für ein Fixture: `Fixture 12 thru 16` ist dasselbe wie `12 thru 16` |
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
| `sequence` | Das Hauptwort für eine Cue-Liste. Allein — `Sequence 2` — wählt sie aus |
| `store` | Schreibt, was der Programmer hält, in eine Cue, eine Gruppe, ein Preset oder einen View |
| `thru` | Ein Bereich: `1 thru 8` |
| `update` | Schreibt Änderungen in die Cue zurück, aus der sie kamen |
| `view` | Das Hauptwort für einen View — eine gespeicherte Anordnung der Leinwand |

## Wenn eine Zeile falsch ist

Das Pult rät nicht. Eine Zeile, die es nicht lesen kann, wird abgelehnt — **mit
dem Grund in der Kommandozeile selbst**, nicht in einem Fenster über der
Leinwand: die Show läuft weiter, nichts ist blockiert, und ein `Go` vom X-Touch
wartet nicht darauf, dass Sie etwas wegklicken.

Würde eine Zeile etwas überschreiben, werden Sie gefragt — wieder in der
Kommandozeile — ob Sie *merge*, *override* oder *cancel* meinten. `Escape` bricht
ab, und ein abgebrochener Dialog ändert **gar nichts**.

## Wie es weitergeht

Das [Handbuch für den Operator](/de/handbuch/operator/) hat das Ganze im
Zusammenhang: wie die Auswahl funktioniert, was der Programmer ist, und was jede
der drei Stufen von `Clear` wirklich wegnimmt.
