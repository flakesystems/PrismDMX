# Issue Report — 2026-10-07

Automatisch erstellter Bericht des Issue-Tracking-Laufs.

---

## Überblick

**Keine neuen GitHub-Issues** seit dem letzten Lauf (2026-09-30).

Seit dem letzten Lauf wurden zwei bisher offene Einträge behoben und in
`docs/ISSUES.md` und den Known-Faults-Seiten bereits vermerkt:

- **B65** — Ein Rig lässt sich nicht als MVR exportieren → ✅ behoben (S64)
- **B68** — Der Mac-Installer enthält keine OFL-Bibliothek (GitHub #41) → ✅ behoben
  (Commit 880d9ec, 2026-10-03)

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

---

## Neue Issues — Dieser Lauf

Keine.

---

## Geschlossene Issues — neue Kommentare geprüft

Issue #41 hatte einen neuen Kommentar seit dem letzten Lauf (2026-10-03): die
Behebung in Commit 880d9ec wurde dort dokumentiert. Der Kommentar war von Claude
(automatisch), kein Hinweis auf ein persistentes Problem.

Alle anderen geschlossenen Issues wurden auf neue Kommentare geprüft — keine
nicht-automatisierten Kommentare, kein Issue wiedereröffnet.

---

## Änderungen an `docs/ISSUES.md`

Keine Änderungen in diesem Lauf — `docs/ISSUES.md` war bereits auf aktuellem Stand.

---

## Bekannte-Fehler-Seiten — Prüfung

`docs/site/known-faults.en.md` und `docs/site/known-faults.de.md` waren bereits
auf aktuellem Stand:

- **B65** und **B68** sind in beiden Seiten als „Fixed for the next release"
  eingetragen.
- **B63** und **B66** sind weiterhin als offen aufgeführt (ohne Fix-Vermerk).

Der Test `the_known_faults_page_lists_the_faults_that_are_open` sollte grün
bleiben: alle offenen ISSUES.md-Einträge (B63, B66) sind auf beiden Seiten
genannt.

---

## Aktueller Stand

**Stand 2026-10-07: 2 offene Einträge** — B63, B66.

- **66 behobene Einträge** (alle außer B63 und B66).
- **68 Einträge gesamt** in `docs/ISSUES.md` (B35 und einige weitere Nummern
  nicht vergeben — Lücken sind laut Datei kein Fehler).
- B63 (Zeichnung des Pults im Controls-Menü) wartet auf eine eigene Session.
- B66 (Mehrere Funktionen auf mehreren Tasten) wird vom Eigentümer selbst
  korrigiert.

---

## Vorheriger Lauf

Der Lauf vom **2026-09-30** hatte B68 (GitHub #41) als neu erfasst.
B63, B65, B66 und B68 waren damals offen. Seither wurden B65 und B68 behoben.
