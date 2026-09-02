# ISSUE_REPORT.md — Automatischer Durchlauf 2026-09-02

Dieser Bericht dokumentiert den Abgleich zwischen den geöffneten, gelabelten
GitHub-Issues und dem Tracking-Dokument `docs/ISSUES.md`.

---

## Durchgeführte Schritte

1. `CLAUDE.md` und `docs/ISSUES.md` gelesen.
2. Alle offenen, gelabelten Issues im Repository `flakesystems/prismdmx`
   abgerufen (2 Issues).
3. Jeden Issue gegen `ISSUES.md` abgeglichen.
4. Neue Einträge angelegt, Kommentare auf GitHub gepostet.

---

## Abgeglichene Issues

### GitHub #1 — Wrongly ordered clear steps

| Feld | Wert |
|---|---|
| Labels | bug |
| Status auf GitHub | offen |
| In ISSUES.md vorher | **nicht vorhanden** |
| Aktion | **Neu angelegt als B37** |

**B37** wurde in der Sektion *Programmer, Presets, Groups* angelegt.
Schwere: ärgerlich. Die Reihenfolge der Clear-Stufen ist falsch: zuerst wird
die Ausgabe gelöscht, dann erst die Fixture-Auswahl — das ist das Gegenteil
des erwarteten Verhaltens und verhindert das parallele Programmieren mehrerer
Fixtures.

---

### GitHub #2 — There are channels missing in the programmer

| Feld | Wert |
|---|---|
| Labels | bug, enhancement |
| Status auf GitHub | offen |
| In ISSUES.md vorher | **nicht vorhanden** |
| Aktion | **Neu angelegt als B38** |

**B38** wurde in der Sektion *Programmer, Presets, Groups* angelegt.
Schwere: ärgerlich. Nicht alle Kanäle von Open-Fixture-Library-Profilen sind
im Programmer einem Attribut zugeordnet; außerdem fehlt die Unterstützung von
OFL-Capabilities vollständig.

---

## Gesamtbild

| ISSUES.md-Eintrag | GitHub-Issue | Ergebnis |
|---|---|---|
| B37 (neu) | #1 | neu angelegt, offen |
| B38 (neu) | #2 | neu angelegt, offen |

Kein Issue war ein Duplikat eines bestehenden Eintrags. Kein Issue, das als
geschlossen getrackt war, wurde auf GitHub geschlossen. Alle anderen
ISSUES.md-Einträge (B1–B36) haben keine direkte GitHub-Issue-Nummer und
wurden in diesem Durchlauf nicht berührt.
