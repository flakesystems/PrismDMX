# Issue Report — 2026-09-07

Automatisch erstellter Bericht des Issue-Tracking-Laufs, hier als erster Schritt
von **S41/S42** durchgeführt: bevor ein Handbuch geschrieben wird, muss
feststehen, was bekannt kaputt ist. Ein Fehler, den ein Handbuch beschreibt, als
wäre er Absicht, ist der teuerste Text in diesem Projekt.

---

## Überblick

**Keine offenen GitHub-Issues.** Alle sechs jemals angelegten sind geschlossen.

| GitHub # | ISSUES.md | Titel | Status |
|---|---|---|---|
| #1 | B37 | (bug) Wrongly ordered clear steps | ✅ geschlossen — behoben in S51 |
| #2 | B38 | (bug) Missing OFL channel mappings and capability support in programmer | ✅ geschlossen — behoben in S51, fortgesetzt in S52–S54 |
| #3 | B39 | (bug) Tray icon refuses to close if the daemon is closed manually | ✅ geschlossen — behoben in S51 |
| #5 | B40 | (bug) Flakey e2e test in looks.spec.ts | ✅ geschlossen |
| #8 | B42 | (feat) Add fullscreen support | ✅ geschlossen — behoben in S51 |
| #9 | B43 | (feat) Add better support for custom fixtures | ✅ geschlossen — behoben in S51 |

Seit `v0.9.1` und `v0.9.2` ist **nichts Neues gemeldet worden**. Das ist kein
Beleg dafür, dass nichts kaputt ist — es ist der Beleg dafür, dass die Beta
bisher **geschlossen** war und wenige Leute darin waren. Genau das ändern S41
und S42.

---

## Was in `docs/ISSUES.md` offen ist

Ein Eintrag, und er ist bewusst offen:

### B52 — Das Pult folgt einem Switching Channel nicht, während die Show läuft

Seit S54 ist ein Switching-Alias **immer erreichbar**, und wo sich alle
Stellungen über den Parameter einig sind, trägt der Slot diesen Parameter. Was
das Pult nicht tut, ist dem Umschalten während der Show zu folgen. Das ist keine
kleine Ergänzung des Readers, sondern eine Frage an das Modell: der Schlüssel,
unter dem eine Cue einen Wert ablegt, dürfte sich nicht ändern, während die Cue
läuft.

**Für diese Session heißt das:** beide Handbücher sagen es ausdrücklich, mit der
Nummer daneben —
[`manual/operator.de.md`](manual/operator.de.md#jeder-kanal-hat-einen-knopf) und
`README.md` unter *was es noch nicht kann*. Kein Text in dieser Session
behauptet, das Pult folge dem Umschalten.

Die beiden anderen `☐` in `docs/ISSUES.md` sind die **Vorlagen**, die die Datei
in ihrem eigenen Kopf als Vorlagen bezeichnet. Sie tragen `Bxx` und keine
Nummer, genau damit sie beim Durchzählen nicht als Einträge mitgezählt werden.

---

## Durchgeführte Aktionen

Keine. Es gab nichts zu schließen, nichts umzubenennen und nichts aufzunehmen.

---

## Vorheriger Lauf

Der Lauf vom **2026-09-05** schloss #5 (B40), passte die Titel von #8 und #9 an
und ergänzte in B38 die beiden OFL-Dokumentationslinks des Eigentümers. Die
Issues #1, #2, #3, #8 und #9 wurden danach durch S51–S54 behoben und
geschlossen.
