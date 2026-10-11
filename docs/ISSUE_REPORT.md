# Issue Report — 2026-10-11

Automatisch erstellter Bericht des Issue-Tracking-Laufs.

---

## Überblick

**Ein neues GitHub-Issue** seit dem letzten Lauf (2026-10-07): **#42** —
3D-Positionskalibrierung für Scheinwerfer.

Keine bisher offenen Einträge wurden seit dem letzten Lauf behoben.

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
| #42 | B70 | (feat) 3D-Positionskalibrierung für Scheinwerfer | ☐ offen |

---

## Neue Issues — Dieser Lauf

### GitHub #42 → B70 — 3D-Positionskalibrierung für Scheinwerfer

- **Label:** enhancement
- **Erstellt:** 2026-10-11
- **Von:** Kuschelcraft
- **Schwere:** ärgerlich
- **ISSUES.md-Eintrag:** B70, neu angelegt im Abschnitt *Canvas, Fenster und Layout*
- **Known-Faults-Seiten:** in beiden Seiten unter *Found while testing the next release* / *Beim Test der nächsten Version gefunden* eingetragen
- **GitHub-Kommentar:** gepostet

Der Eintrag beschreibt das Fehlen eines Kalibrierungs-Workflows für die
3D-Positionierung von Scheinwerfern im virtuellen Rig — vergleichbar mit dem
Ablauf in grandMA3-Konsolen. Die Funktion ist noch nicht implementiert.

---

## Geschlossene Issues — neue Kommentare geprüft

Alle zuvor geschlossenen Issues wurden auf neue nicht-automatisierte Kommentare
geprüft — keine gefunden, kein Issue wiedereröffnet.

---

## Änderungen an `docs/ISSUES.md`

- **B70** neu angelegt (☐ offen) für GitHub #42

---

## Bekannte-Fehler-Seiten — Prüfung

`docs/site/known-faults.en.md` und `docs/site/known-faults.de.md` wurden
aktualisiert:

- **B70** in beiden Seiten unter dem Abschnitt „Beim Test der nächsten Version
  gefunden" eingetragen.
- **B63** und **B66** bleiben weiterhin offen und sind auf beiden Seiten
  aufgeführt.

Der Test `the_known_faults_page_lists_the_faults_that_are_open` sollte grün
bleiben: alle offenen ISSUES.md-Einträge (B63, B66, B70) sind auf beiden Seiten
genannt.

---

## Aktueller Stand

**Stand 2026-10-11: 3 offene Einträge** — B63, B66, B70.

- **66 behobene Einträge**.
- **69 Einträge gesamt** in `docs/ISSUES.md` (B35 und einige weitere Nummern
  nicht vergeben — Lücken sind laut Datei kein Fehler).
- B63 (Zeichnung des Pults im Controls-Menü) wartet auf eine eigene Session.
- B66 (Mehrere Funktionen auf mehreren Tasten) wird vom Eigentümer selbst
  korrigiert.
- B70 (3D-Positionskalibrierung) ist ein Feature-Request, noch nicht
  implementiert.

---

## Vorheriger Lauf

Der Lauf vom **2026-10-07** hatte keine neuen Issues. B65 und B68 waren damals
als behoben vermerkt. Stand zu dem Zeitpunkt: 2 offene Einträge (B63, B66).
