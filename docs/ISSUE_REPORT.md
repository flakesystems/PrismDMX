# Issue Report — 2026-09-09

## Überblick

Dieser Lauf hat **2 offene, gelabelte GitHub Issues** geprüft und **2 neue Einträge** in `docs/ISSUES.md` aufgenommen. Kein geschlossener Issue trägt einen neuen Kommentar, der auf ein fortbestehendes Problem hindeutet.

---

## Verarbeitete Issues

### GitHub #21 — Input Selector Tests to restricive

**Status:** Neu aufgenommen als **B53**  
**Titel aktualisiert:** `(bug) Input selector validation too restrictive during editing`

Der Issue war nicht in `docs/ISSUES.md` erfasst. Inhalt: Required Input-Felder verhindern das vollständige Leeren während der Eingabe, was das Ändern der ersten Stelle einer Zahl oder des ersten Buchstabens eines Worts blockiert. Die Validierung soll nur beim Apply greifen, nicht während der Eingabe.

**Aufgenommen als B53** im Abschnitt *Sonstiges* — Status: ☐ offen.  
GitHub-Kommentar und Titelkorrektur wurden gepostet.

---

### GitHub #9 — (feat) Add better support for custom fixtures

**Status:** Teilweise behoben (B43 ✅), neuer Sub-Issue aufgenommen als **B54**

B43 in `docs/ISSUES.md` ist als ✅ behoben in S51 markiert: eigene Fixture-Profile können unter `fixtures/` im Datenverzeichnis abgelegt werden, werden vom Daemon mitgelesen und im Auswahlfenster als `yours` gekennzeichnet.

Ein neuer Kommentar des Eigentümers vom 2026-09-07 meldet jedoch: *"The folder doesnt get created on installation. This can be confusing for users"*. Dieses Sub-Issue ist von B43 unabhängig und wurde als **B54** aufgenommen.

Issue #9 bleibt offen, da B54 noch unbehoben ist.  
GitHub-Kommentar wurde mit dem neuen Tracking-Eintrag gepostet.

---

## Geschlossene Issues

Alle fünf geschlossenen Issues (#1, #2, #3, #5, #8) wurden geprüft. Keiner hat neue Kommentare von Nicht-Claude-Quellen, die auf ein fortbestehendes Problem hinweisen. Kein Wiedereröffnen erforderlich.

---

## Neue Einträge in ISSUES.md

| Eintrag | GitHub | Schwere   | Status  | Beschreibung |
|---------|--------|-----------|---------|--------------|
| B53     | #21    | ärgerlich | ☐ offen | Input-Felder lassen sich während der Eingabe nicht vollständig leeren |
| B54     | #9     | ärgerlich | ☐ offen | Das `fixtures/`-Verzeichnis wird bei der Installation nicht angelegt |

---

## Ausstehende offene Einträge (Gesamtübersicht nach diesem Lauf)

| Eintrag | Beschreibung |
|---------|-------------|
| B52     | Das Pult folgt einem Switching Channel nicht, während die Show läuft |
| B53     | Input-Felder lassen sich während der Eingabe nicht vollständig leeren (GitHub #21) |
| B54     | Das fixtures/-Verzeichnis wird bei der Installation nicht angelegt (GitHub #9) |
