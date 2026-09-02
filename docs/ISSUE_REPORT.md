# Issue Report — 2026-09-02

Automatisch erstellter Bericht des Scheduled Issue-Tracking-Laufs.

---

## Überblick

| GitHub # | ISSUES.md | Titel (nach Umbenennung) | Status |
|---|---|---|---|
| #1 | B37 | (bug) Wrongly ordered clear steps | ☐ offen — bereits erfasst |
| #2 | B38 | (bug) Missing OFL channel mappings and capability support in programmer | ☐ offen — bereits erfasst |
| #3 | B39 | (bug) Tray icon refuses to close if the daemon is closed manually | ☐ offen — **neu aufgenommen** |
| #5 | B40 | (bug) Flakey e2e test in looks.spec.ts | ☐ offen — **neu aufgenommen** |

Keine geschlossenen Issues vorhanden. Keine Duplikate gefunden.

---

## Durchgeführte Aktionen

### Bereits erfasste Issues (#1, #2)

- **#1 / B37:** Eintrag war korrekt als offen markiert. Kein neuer Kommentar nötig (Tracking-Kommentar aus dem Vorlauf vorhanden). Titel auf `(bug) Wrongly ordered clear steps` aktualisiert.
- **#2 / B38:** Eintrag war korrekt als offen markiert. Der vom Eigentümer hinterlassene Kontext-Kommentar (OFL-Links) war bereits in den B38-Eintrag integriert. Kein weiterer Kommentar nötig. Titel auf `(bug) Missing OFL channel mappings and capability support in programmer` aktualisiert.

### Neu aufgenommene Issues (#3, #5)

#### B39 — Tray-Icon schließt nicht, wenn Daemon manuell beendet wird (GitHub #3)

- In `docs/ISSUES.md` unter **Sonstiges** als B39 eingetragen, Schwere: ärgerlich.
- Tracking-Kommentar auf GitHub gepostet.
- Titel auf `(bug) Tray icon refuses to close if the daemon is closed manually` aktualisiert.

**Beschreibung:** Wird der Daemon außerhalb des Tray-Menüs beendet (z. B. via Task-Manager), bleibt das Tray-Icon stehen. "Stop the desk" findet keinen Prozess. Ein neuer Daemon-Start erzeugt ein zweites Icon.

#### B40 — Instabiler e2e-Test in looks.spec.ts (GitHub #5)

- In `docs/ISSUES.md` unter **Sonstiges** als B40 eingetragen, Schwere: ärgerlich.
- Tracking-Kommentar auf GitHub gepostet.
- Titel auf `(bug) Flakey e2e test in looks.spec.ts` aktualisiert.

**Beschreibung:** `ui/e2e/looks.spec.ts:374` — `a preset link is alive: editing the preset changes the light a cue puts out` schlägt bei Docs-only-CI-Runs fehl. Da ein Docs-only-Run keine UI-Artefakte neu baut, deutet das auf eine Race Condition oder Timing-Abhängigkeit im Test selbst hin.

---

## Keine Aktion erforderlich

- Keine geschlossenen Issues mit neuen Kommentaren gefunden.
- Keine Duplikate identifiziert.
- Issues #1 und #2 hatten keine neuen Kommentare (außer dem bereits vorhandenen Claude-Tracking-Kommentar).
