# design/skeleton — wie die Oberfläche angeordnet sein soll

Ein Penpot-Entwurf des Eigentümers, exportiert. Er ist die zweite Hälfte der
Anforderung von **S43** (`IMPLEMENTATION_PLAN.md`) und sagt zwei Dinge:

- **den Fluss** — was zu was führt, und wodurch: welche Taste, welches Kommando,
  welches Ereignis;
- **die Anordnung** — welche Bereiche es gibt, wie sie zueinander liegen, was
  groß und was klein ist.

Er sagt **ausdrücklich keine Details**: keine exakten Pixel, keine Schriftgrößen,
keine Farbwerte, keine Beschriftungstexte im Wortlaut. Was er nicht sagt,
entscheidet die Session — und schreibt die Entscheidung auf. Was er sagt, gilt;
jede bewusste Abweichung davon wird mit ihrem Grund notiert
(`ARCHITECTURE_SPEC.md`).

---

## Die Boards

Eine Zeile pro Board. Die Spalte **Was es zeigt** ist die wichtigste: ein
Dateiname sagt nicht, worauf man beim Ansehen achten soll.

| Datei | Board | Was es zeigt |
|---|---|---|
| `canvas-default.pdf` | *Canvas — Grundansicht* | *Was nach dem Start offen ist, und wo. Beispielzeile — überschreiben oder löschen* |
| | | |
| | | |
| | | |

## Die Pfeile im Flowchart

Ein Pfeil ohne Legende ist die eine Stelle, an der zuverlässig etwas anderes
gelesen wird als gemeint war. Also: was bedeutet welche Pfeilart, und was
bedeutet die Beschriftung an einem Pfeil?

| Pfeil | Bedeutet |
|---|---|
| *durchgezogen* | *z. B.: ein Fensterwechsel — danach ist etwas anderes zu sehen* |
| *gestrichelt* | *z. B.: ein Zustandswechsel ohne Fensterwechsel — dasselbe Fenster zeigt etwas anderes an* |
| *Beschriftung am Pfeil* | *z. B.: die Taste, das Kommando oder das Ereignis, das ihn auslöst* |

*(Die drei Zeilen sind Vorschläge, damit die Tabelle nicht leer dasteht. Wenn im
Entwurf nur eine Pfeilart vorkommt, reicht eine Zeile.)*

## Was ein Kasten im Entwurf ist

Damit klar ist, was übernommen werden soll und was nur Papier ist:

| Im Entwurf | Heißt |
|---|---|
| *ein Kasten mit Titel* | *ein Bereich der Oberfläche, der so existieren soll* |
| *ein Kasten ohne Titel* | *Platzhalter für Inhalt — die Größe zählt, der Inhalt nicht* |
| *grau / durchgestrichen* | *ist gemeint, aber nicht für dieses Release* |

## Export-Konvention

Damit ein Nachtrag genauso aussieht wie der erste Export, und damit die Session
beides lesen kann:

- **Pro Board eine Datei**:
  - **PDF** — das, was tatsächlich *angesehen* wird. Anordnung und Fluss
    erfasst man als Bild.
- **Dateinamen** klein, mit Bindestrich, nach dem, was zu sehen ist, nicht nach
  der Reihenfolge im Werkzeug: `canvas-default`, `flow-windows`,
  `executor-strip` — nicht `board-1`.
- **Kein `.penpot`-Export.** Das ist ein ZIP mit internem JSON; lesbar, aber
  schlechter als PNG + SVG.
- **Ein geändertes Board wird unter demselben Namen neu exportiert**, beide
  Dateien, und die Zeile in der Tabelle oben wird nachgezogen, falls sich
  geändert hat, *was* es zeigt.

## Wenn etwas im Entwurf und im Programm nicht zusammenpassen

Dann ist das eine Frage und keine Entscheidung: es wird gefragt, in dem Moment,
in dem es auffällt. Zwei Fälle sind besonders wahrscheinlich, und beide sind
Regeln, die schwerer wiegen als eine Zeichnung —

- **Nichts scrollt außerhalb der Canvas** (`CLAUDE.md`). Eine Anordnung, die bei
  1280 × 720 nicht aufgeht, wird nicht durch eine scrollende Seite gerettet.
- **Die Kommandozeile ist *die* Oberfläche, nicht eine von zweien**
  (`ARCHITECTURE_SPEC.md` §4.5). Ein Entwurf, in dem sie eine Nebenrolle spielt,
  ist eine Frage wert, bevor er umgesetzt wird.
- **Keine ungestylten Buttons** Alle Buttons müssen einen Style haben. Es darf keine default HTML Buttons geben. **Das gilt auch für andere Form Elemente**
- Es sind noch keine Fenster fertig designed, trotzdem sollten dort die anderen Vorgaben erfüllt werden.
- Der Style des Patch Fensters muss grundlegend überarbeitet werden. Dabei soll die Fixture Liste im Vordergrund stehen, die klar abgetrennte Spalten haben sollte, um die Übersichtlichkeit zu erhöhen.