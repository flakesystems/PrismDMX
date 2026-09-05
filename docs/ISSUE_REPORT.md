# Issue Report — 2026-09-05

Automatisch erstellter Bericht des Scheduled Issue-Tracking-Laufs.

---

## Überblick

| GitHub # | ISSUES.md | Titel (nach Umbenennung) | Status |
|---|---|---|---|
| #1 | B37 | (bug) Wrongly ordered clear steps | ☐ offen — keine Änderung |
| #2 | B38 | (bug) Missing OFL channel mappings and capability support in programmer | ☐ offen — OFL-Doku-Links ergänzt |
| #3 | B39 | (bug) Tray icon refuses to close if the daemon is closed manually | ☐ offen — keine Änderung |
| #5 | B40 | (bug) Flakey e2e test in looks.spec.ts | ✅ behoben — **GitHub-Issue geschlossen** |
| #8 | B42 | (feat) Add fullscreen support | ☐ offen — **neu: Titel aktualisiert, Tracking-Kommentar gepostet** |
| #9 | B43 | (feat) Add better support for custom fixtures | ☐ offen — **neu: Titel aktualisiert, Tracking-Kommentar gepostet** |

Keine geschlossenen Issues vorhanden. Keine Duplikate gefunden.

---

## Durchgeführte Aktionen

### Issue #5 / B40 — geschlossen

B40 war in `docs/ISSUES.md` bereits als ✅ behoben markiert, GitHub-Issue #5 stand aber noch offen. Schließungskommentar mit Begründung gepostet, Issue als *completed* geschlossen.

**Ursachen des Fehlers (aus ISSUES.md B40):** Zwei Race Conditions im Command-Line-Dispatch seit S49 plus ein nicht-wartendes `count()` im Playwright-Test:
1. `useMirror.ran` ersetzt `cancel` — verhindert, dass das Daemon-Leeren nach Ausführen als fremde Neuigkeit übernommen wird.
2. `pick` vergleicht das Feld beim Eintreffen der Antwort — überschreibt nicht mehr die seither getippte Zeile.
3. `store()` wartet auf einen der beiden Daemon-Zustände (Frage steht / Feld leer).

Tests: `keeps a line typed while a key's line is still in flight`, `does not let a pick decided late overwrite the line typed since` (`ui/src/desk/desk.test.tsx`).

### Issues #8 und #9 — neu aufgenommen und Titel angepasst

Beide Issues wurden vom Eigentümer mit dem Label `enhancement` versehen (Priority: Medium). Die Einträge **B42** und **B43** waren in `docs/ISSUES.md` bereits vorhanden (aus einem früheren Session-Commit). Die Titel wurden auf das `(type) Short explanation`-Format angepasst und Tracking-Kommentare auf GitHub gepostet.

#### B42 — Kein Vollbild (GitHub #8)
- Titel: `Add fullscreen support` → `(feat) Add fullscreen support`
- Tracking-Kommentar gepostet.

#### B43 — Für eigene Fixture-Profile gibt es keinen Ort (GitHub #9)
- Titel: `Add better support for custom fixtures` → `(feat) Add better support for custom fixtures`
- Tracking-Kommentar gepostet.

### Issue #2 / B38 — OFL-Dokumentationslinks ergänzt

Der Eigentümer hatte unter Issue #2 zwei Referenz-Links zur OFL-Dokumentation hinterlassen (Fixture Format und Capability Types). Diese wurden in den B38-Eintrag in `docs/ISSUES.md` unter **Referenzen** aufgenommen, da sie für die Implementierung direkt relevant sind.

---

## Keine Aktion erforderlich

- **#1 / B37:** Offen, kein neuer Kommentar, kein Fortschritt.
- **#3 / B39:** Offen, kein neuer Kommentar, kein Fortschritt.
- Keine geschlossenen Issues mit neuen Kommentaren.
- Keine Duplikate identifiziert.

---

## Vorheriger Lauf

Der Lauf vom 2026-09-02 hatte B39 und B40 erstmals aufgenommen und deren Tracking-Kommentare auf GitHub gepostet. B40 wurde seitdem behoben; dieser Lauf schließt den Issue entsprechend.
