# Issue Report — 2026-09-16

Automatisch erstellter Bericht des Issue-Tracking-Laufs.

---

## Überblick

**10 neue GitHub-Issues** wurden seit dem letzten Lauf (2026-09-07) eröffnet und
gelabelt. Alle wurden in `docs/ISSUES.md` aufgenommen.

| GitHub # | ISSUES.md | Titel | Status |
|---|---|---|---|
| #9 | B43 + B54 | (feat) Add better support for custom fixtures | offen — B43 ✅ behoben, B54 ☐ offen (Verzeichnis wird nicht angelegt) |
| #21 | B53 | (bug) Input selector validation too restrictive during editing | ☐ offen |
| #23 | B55 | (bug) Controls Menu crashes when binding a new control | ☐ offen |
| #24 | B56 | (bug) View change clears the command line | ☐ offen |
| #25 | B57 | (bug) Window focus prevents selecting on first click | ☐ offen |
| #26 | B58 | (feat) Oops key deletes command line words | ☐ offen |
| #27 | B59 | (bug) Crossfade fader progress not synced between clients | ☐ offen |
| #28 | B60 | (feat) Improve Patch window UI and UX | ☐ offen |
| #29 | B61 | (bug) Command feedback moves the UI | ☐ offen |
| #30 | B62 | (feat) Remove store bar from Cue Viewer | ☐ offen |

---

## Geschlossene Issues — neue Kommentare geprüft

| GitHub # | ISSUES.md | Befund |
|---|---|---|
| #1 | B37 | Keine neuen Kommentare |
| #2 | B38 | Keine neuen Kommentare |
| #3 | B39 | Keine neuen Kommentare |
| #5 | B40 | Keine neuen Kommentare |
| #8 | B42 | Keine neuen Kommentare |

---

## Neue Einträge in `docs/ISSUES.md`

### B53 — Pflichtfelder lassen das vollständige Leeren nicht zu (GitHub #21)

War in einem Kommentar vom 2026-09-09 erwähnt worden, fehlte aber im Dokument.
Nachgetragen unter **Sonstiges**.

### B54 — `fixtures/`-Verzeichnis wird nicht angelegt (GitHub #9, Sub-Issue)

Ein Kommentar vom 2026-09-07 meldete, dass das Verzeichnis nach einer frischen
Installation nicht existiert. In ISSUES.md unter **Patch und Fixture Sheet**
aufgenommen. Issue #9 bleibt offen bis B54 behoben ist.

### B55 — Controls-Menü bricht zusammen (GitHub #23)

Blocker: Client trennt sich bei jeder Tastenbindung. Aufgenommen unter
**Einstellungen und Pult**.

### B56 — Ansichtswechsel löscht Kommandozeile (GitHub #24)

Aufgenommen unter **Kommandozeile**.

### B57 — Fensterfokus verhindert Auswahl beim ersten Klick (GitHub #25)

Aufgenommen unter **Canvas, Fenster und Layout**.

### B58 — Oops löscht keine Wörter aus der Kommandozeile (GitHub #26)

Aufgenommen unter **Kommandozeile**.

### B59 — Crossfade-Fortschritt nicht synchronisiert (GitHub #27)

Aufgenommen unter **Executors und Wiedergabe**.

### B60 — Patch-Fenster: mehrere Fehler und Verbesserungen (GitHub #28)

Fasst acht gemeldete Probleme aus Issue #28 zusammen. Aufgenommen unter
**Patch und Fixture Sheet**.

### B61 — Befehlsfeedback verschiebt das Layout (GitHub #29)

Aufgenommen unter **Canvas, Fenster und Layout**.

### B62 — Store-Leiste im Cue Viewer entfernen (GitHub #30)

Aufgenommen unter **Executors und Wiedergabe**.

---

## Aktueller Stand

**Stand 2026-09-20: keine offenen Einträge.** Die elf, die dieser Lauf zählte —
B52 bis B62 — sind in S56, S57 und S58 behoben, und die zehn GitHub-Issues
(#9, #21, #23–#30) sind mit dem Commit, der sie behoben hat, geschlossen. Die
Korrekturen erscheinen als **0.9.3**.

**Dieser Lauf zählte 11 offene Einträge:** B52, B53, B54, B55, B56, B57, B58,
B59, B60, B61, B62.

**50 behobene Einträge.** Alle anderen B-Nummern von B1–B51 sowie B53–B62 mit
Ausnahme der elf oben genannten sind behoben.

`docs/site/known-faults.de.md` und `docs/site/known-faults.en.md` wurden
aktualisiert. Der Corpus-Test `the_known_faults_page_lists_the_faults_that_are_open`
prüft, dass jede offene B-Nummer in beiden Seiten vorkommt.

---

## Titel-Aktualisierungen auf GitHub

Für die neuen Issues #23–#30 wurden Titel im Format `(type) Kurzbeschreibung`
gesetzt und ein Tracking-Kommentar hinterlassen.

---

## Vorheriger Lauf

Der Lauf vom **2026-09-07** stellte fest, dass alle sechs damals bekannten
Issues geschlossen waren und B52 der einzige offene Eintrag in `docs/ISSUES.md`
war.
