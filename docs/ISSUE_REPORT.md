# Issue Report — 2026-09-23

Automatisch erstellter Bericht des Issue-Tracking-Laufs.

---

## Überblick

**Keine neuen GitHub-Issues** seit dem letzten Lauf (2026-09-20). Alle 15 Issues
im Repository sind geschlossen.

---

## GitHub-Issues — aktueller Stand

| GitHub # | ISSUES.md | Titel | Status |
|---|---|---|---|
| #1 | B37 | (bug) Wrongly ordered clear steps | ✅ geschlossen |
| #2 | B38 | (bug) Missing OFL channel mappings and capability support | ✅ geschlossen |
| #3 | B39 | (bug) Tray icon refuses to close if the daemon is closed manually | ✅ geschlossen |
| #5 | B40 | (bug) Flakey e2e test in looks.spec.ts | ✅ geschlossen |
| #8 | B42 | (feat) Add fullscreen support | ✅ geschlossen |
| #9 | B43 + B54 | (feat) Add better support for custom fixtures | ✅ geschlossen |
| #21 | B53 | (bug) Input selector validation too restrictive during editing | ✅ geschlossen |
| #23 | B55 | (bug) Controls Menu crashes when binding a new key | ✅ geschlossen |
| #24 | B56 | (bug) View change clears the command line | ✅ geschlossen |
| #25 | B57 | (bug) Window focus prevents selecting on first click | ✅ geschlossen |
| #26 | B58 | (feat) Oops key deletes command line words | ✅ geschlossen |
| #27 | B59 | (bug) Crossfade fader progress not synced between clients | ✅ geschlossen |
| #28 | B60 | (feat) Improve Patch window UI and UX | ✅ geschlossen |
| #29 | B61 | (bug) Command feedback moves the UI | ✅ geschlossen |
| #30 | B62 | (feat) Remove store bar from Cue Viewer | ✅ geschlossen |

---

## Geschlossene Issues — neue Kommentare geprüft

Alle geschlossenen Issues wurden auf neue Kommentare (nicht von Claude) geprüft.

| GitHub # | ISSUES.md | Befund |
|---|---|---|
| #2 | B38 | Keine neuen Kommentare |
| #9 | B43/B54 | Letzte Kommentare vom 2026-09-07 (Nutzer) und 2026-09-20 (Lauf) — behoben, kein neues Problem |
| alle anderen | — | Keine neuen Kommentare |

Kein geschlossener Issue wurde durch neue Kommentare als persistent-problematisch identifiziert.

---

## Änderungen an `docs/ISSUES.md`

Keine Änderungen erforderlich. Das Dokument ist aktuell:

- **B64** (GDTF-Bibliothek: Pult startet nicht) und **B67** (Viewer 3D: drei Fehler)
  wurden am 2026-09-22 behoben und sind in `ISSUES.md` so vermerkt. Beide
  wurden nicht in die Known-Faults-Seiten aufgenommen, da sie vor Veröffentlichung
  der nächsten Version bereits behoben waren.

---

## Bekannte-Fehler-Seiten — Prüfung

`docs/site/known-faults.en.md` und `docs/site/known-faults.de.md` sind aktuell
und stimmen mit `docs/ISSUES.md` überein. Keine Änderungen erforderlich.

**Offene Einträge (3):**

| B-Nr. | Titel | Seiten |
|---|---|---|
| B63 | Zeichnung des Pults im Controls-Menü falsch | ✅ in beiden Seiten |
| B65 | Rig lässt sich nicht als MVR exportieren | ✅ in beiden Seiten |
| B66 | Mehrere Funktionen auf mehreren Tasten | ✅ in beiden Seiten |

Der Corpus-Test `the_known_faults_page_lists_the_faults_that_are_open` sollte grün sein.

---

## Aktueller Stand

**Stand 2026-09-23: 3 offene Einträge** — B63, B65, B66.

- **62 behobene Einträge** (B1–B62, B64, B67).
- B63 und B65 warten auf eine Entscheidung des Eigentümers (eigene Session bzw.
  Klärung der OFL-Frage vor dem MVR-Export).
- B66 wird vom Eigentümer selbst korrigiert; ob die mitgelieferte Grundbelegung
  danach angepasst wird, entscheidet er.

---

## Vorheriger Lauf

Der Lauf vom **2026-09-20** stellte fest, dass alle 11 damals offenen Einträge
(B52–B62) in S56, S57 und S58 behoben waren und alle zugehörigen GitHub-Issues
geschlossen wurden. Seit dem 2026-09-20 wurden B64 und B67 behoben (2026-09-22).
