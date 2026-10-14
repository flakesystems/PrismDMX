# Issue Report — 2026-10-14

Automatisch erstellter Bericht des Issue-Tracking-Laufs.

---

## Überblick

**Ein GitHub-Issue geschlossen** seit dem letzten Lauf (2026-10-11): **#42** —
3D-Positionskalibrierung für Scheinwerfer (B70 — behoben in der Session vom 2026-10-11).

Keine neuen Issues seit dem letzten Lauf.

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
| #41 | B68 | (bug) Mac installer doesn't carry the OFL | ✅ geschlossen |
| #42 | B70 | (feat) 3D-Positionskalibrierung für Scheinwerfer | ✅ geschlossen |

---

## Dieser Lauf — Geschlossene Issues

### GitHub #42 → B70 — 3D-Positionskalibrierung für Scheinwerfer

- **Label:** enhancement
- **ISSUES.md-Eintrag:** B70 — ✅ behoben (seit dem Calibration-Commit auf master, 2026-10-11)
- **GitHub-Kommentar:** gepostet
- **GitHub-Issue:** geschlossen

B70 war im letzten Lauf (2026-10-11) als offen gemeldet. Zwischen den Läufen wurde die
Kalibrierungsfunktion implementiert und auf `master` gemergt; `docs/ISSUES.md` trägt den
Eintrag jetzt als behoben. GitHub-Issue #42 wurde entsprechend geschlossen.

**Was umgesetzt wurde:** *Calibrate* im Viewer 3D: Marks als Show-Daten, Aim/Take-Workflow,
Levenberg–Marquardt-Solver, Apply als ein Oops-Schritt, Qualitätsanzeige mit Ampel.
Dokumentation: Bedienhandbuch Kapitel 8. Tests: `calibration.rs`, `ui/e2e/calibration.spec.ts`.

**Noch offen am Rig:** Kalibrierung noch nicht an einem echten Moving Head gefahren (`PROGRESS.md` §5).

---

## Geschlossene Issues — neue Kommentare geprüft

Alle zuvor geschlossenen Issues (außer #42, das in diesem Lauf geschlossen wurde) wurden auf
neue nicht-automatisierte Kommentare geprüft — keine gefunden, kein Issue wiedereröffnet.

---

## Änderungen an `docs/ISSUES.md`

Keine Änderungen in diesem Lauf. B70 wurde bereits als Teil des Calibration-Commits auf
`master` als behoben eingetragen.

---

## Bekannte-Fehler-Seiten — Prüfung

`docs/site/known-faults.en.md` und `docs/site/known-faults.de.md` wurden geprüft — keine
Änderungen erforderlich:

- **B63** und **B66** sind weiterhin offen und auf beiden Seiten aufgeführt ✓
- **B70** ist auf beiden Seiten als „Fixed for the next release" / „In der nächsten Version
  behoben" eingetragen ✓ (passt zum Status in ISSUES.md)

Der Test `the_known_faults_page_lists_the_faults_that_are_open` sollte grün bleiben: die
einzigen offenen ISSUES.md-Einträge (B63, B66) sind auf beiden Seiten genannt.

---

## Aktueller Stand

**Stand 2026-10-14: 2 offene Einträge** — B63, B66.

- **67 behobene Einträge** (B70 kommt neu dazu).
- **69 Einträge gesamt** in `docs/ISSUES.md`.
- B63 (Zeichnung des Pults im Controls-Menü) wartet auf eine eigene Session.
- B66 (Mehrere Funktionen auf mehreren Tasten) wird vom Eigentümer selbst korrigiert.

---

## Vorheriger Lauf

Der Lauf vom **2026-10-11** hatte ein neues Issue (#42 → B70). Stand zu dem Zeitpunkt:
3 offene Einträge (B63, B66, B70).
