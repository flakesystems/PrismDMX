# UI-Redesign — Einstieg

Dieser Branch ist der Startpunkt für die neue Oberfläche. Alles, was mit dem
Daemon (`prismd`, Rust) zu tun hat, ist fertig verdrahtet und liegt in
`src/core/`. Du arbeitest in `src/app/` — nur TypeScript und CSS.

## 1. Einmalig einrichten

Voraussetzungen: **Node 22+** und **Rust** (<https://rustup.rs>, Standard-Optionen).
Rust brauchst du nur, damit die Skripte den Daemon bauen und die Typen erzeugen
können — schreiben musst du keine Zeile davon.

```bash
cd ui
npm run setup
```

Das installiert die npm-Pakete, erzeugt `src/bindings/` aus Rust und baut den
Daemon einmal. Kann jederzeit erneut ausgeführt werden.

## 2. Täglich

| Befehl | Was es tut |
| --- | --- |
| `npm run dev:all` | Daemon **und** Oberfläche in einem Terminal. Adresse aus der Ausgabe öffnen (meist <http://localhost:5173>). Strg+C beendet beides. |
| `npm run dev:all -- --fresh` | Wie oben, aber mit frischer Demo-Show. |
| `npm run dev` | Nur die Oberfläche (wenn der Daemon schon läuft). |
| `npm run daemon` | Nur der Daemon. |
| `npm run bindings` | Typen neu erzeugen — nach einem `git pull`, der `crates/` geändert hat, oder wenn TypeScript `../bindings` nicht findet. |
| `npx tsc -b` · `npm run lint` · `npm run build` | Das, was CI bei jedem Push prüft. |

Der Entwicklungs-Daemon ist harmlos: Mock-Ausgang (sendet kein DMX), eigenes
Datenverzeichnis `ui/.dev-data/` (von git ignoriert), und eine Kopie einer
Demo-Show mit 4 Scheinwerfern. Eine installierte PrismDMX auf demselben Rechner
stört er nicht — außer sie belegt schon Port 7373; dann vorher beenden.

## 3. Aufbau

```text
ui/
├── src/
│   ├── main.tsx          startet Verbindung + React (selten anfassen)
│   ├── bindings/         GENERIERT aus Rust — nicht bearbeiten, nicht committen
│   ├── core/             Daemon-Anbindung, fertig und getestet
│   │   ├── index.ts      ← die EINE Import-Stelle für alles
│   │   ├── hooks.ts      useShow, useSession, useProgrammer, useRunLine, …
│   │   ├── model/        reine Hilfsfunktionen (Fenster, Views, Programmer, …)
│   │   └── ipc/ store/ mirror/ log/ shell/   Interna
│   └── app/              ← DEIN Bereich
│       ├── App.tsx       die vier Bänder
│       ├── layout/       Header, Canvas, WindowFrame, CommandLine, ProgrammerBand, …
│       ├── windows/      Fensterinhalte + registry.tsx (Fenstertyp → Komponente)
│       └── styles/       theme.css (Farben/Größen), base.css, layout.css
├── scripts/              die npm-Skripte oben (Node, laufen auch unter Windows)
└── legacy/               die alte Oberfläche, NUR zum Nachschlagen (wird nicht gebaut)
```

`legacy/` wird weder typgeprüft noch gelintet noch gebaut. Lösch den Ordner,
sobald du ihn nicht mehr brauchst. Auf `master` ist die alte UI ohnehin erhalten.

## 4. Wie Daten fließen

```text
  Daemon ──snapshot + deltas──▶ store ──hooks──▶ deine Komponente
     ▲                                                 │
     └──────────── Command (useSend / useRunLine) ◀────┘
```

**Die Oberfläche hält keinen eigenen Zustand.** Ein Klick ändert nichts direkt,
sondern schickt dem Daemon einen Befehl; der Daemon antwortet mit einer
Änderung, und der Store aktualisiert alle Bildschirme (auch das X-Touch). Auch
offene Fenster, aktive View, Encoder-Bank und die Kommandozeile selbst sind
Daemon-Zustand („Session“).

### Lesen

```tsx
import { listOf, useShow } from "../../core";

export function FixtureList() {
  const show = useShow();              // null, solange keine Verbindung besteht
  if (show === null) return null;
  return (
    <ul>
      {listOf(show.fixtures).map((f) => (
        <li key={f.id}>{f.id} {f.name}</li>
      ))}
    </ul>
  );
}
```

| Hook | Liefert |
| --- | --- |
| `useConnectionStatus()` / `useConnected()` | Verbindungszustand |
| `useShow()` | `ShowDocument`: `fixtures`, `fixtureTypes`, `groups`, `presets`, `sequences` (Cuelisten), `executors`, `marks` — jeweils Objekt `{ "1": …, "2": … }`, mit `listOf()` als sortierte Liste |
| `useSession()` | `SessionDocument`: `session` (offene Fenster, aktive View, Kommandozeile, Encoder-Bank, gewählter Executor, …) und `views` |
| `useProgrammer()` | Selektion, berührte Werte, Clear-Stufe |
| `useNotices()` | Meldungen an den Bediener |
| `useOutputs()`, `useHealth()`, `useShowFile()`, `useUnsavedChanges()`, `useMachine()` | Status-Infos |
| `useDesk(selector)` | Alles andere — Selector **außerhalb** der Komponente definieren |
| `useAsk()` | Fragen an den Daemon, die nicht im Zustand stehen (z. B. Bibliothek durchsuchen) |

Alle Typen (`Fixture`, `Cue`, `Sequence`, `Command`, `WindowType`, …) kommen
ebenfalls aus `../../core`. Sie werden aus Rust erzeugt — sie sind also immer
genau das, was der Daemon sendet.

### Ändern

Zwei Wege:

```tsx
const run = useRunLine();
run("Fixture 1 Thru 4");    // genau wie getippt + Enter
run("At 50");
run("Store Cue 3");
run("Clear");

const send = useSend();
send({ t: "SelectView", viewId: 2 });          // strukturierter Befehl
send({ t: "OpenWindow", window: "Groups" });
```

Die Kommandozeile wird im Daemon geparst; die Oberfläche muss die Syntax nicht
kennen. Alle strukturierten Befehle stehen in `src/bindings/Command.ts`
(TypeScript schlägt sie beim Tippen von `t: "` vor).

### Ein Fenster bauen

1. Komponente in `src/app/windows/` anlegen, Props: `WindowProps`.
2. In `src/app/windows/registry.tsx` beim passenden `WindowType` eintragen.

`GroupsWindow.tsx` und `StatusWindow.tsx` sind kleine Beispiele.
Position/Größe eines Fensters liegt im 1920×1080-Raster der Session;
`asStyle()` macht daraus Prozentwerte. Verschieben/Größe ändern ist der Befehl
`PlaceWindow` (die alte Logik dazu: `legacy/src/canvas/`, Geometrie-Helfer in
`core/model/geometry.ts`).

## 5. Styling — und Tailwind?

Derzeit **reines CSS**: alle Farben, Größen und Abstände sind CSS-Variablen in
`src/app/styles/theme.css`. Das bleibt so nutzbar, egal wie du dich entscheidest.

Falls du Tailwind willst (v4, ohne Konfigurationsdatei):

```bash
npm install -D tailwindcss @tailwindcss/vite
```

- `vite.config.ts`: `import tailwindcss from "@tailwindcss/vite"` und in
  `plugins: [react(), tailwindcss()]` eintragen.
- Oben in `src/app/styles/theme.css`: `@import "tailwindcss";` und die
  Variablen aus `:root` in einen `@theme { … }`-Block übernehmen
  (z. B. `--color-panel: #1c1f25;` → Klasse `bg-panel`).

Zur Abwägung: Tailwind landet nicht im ausgelieferten Programm (nur das
erzeugte CSS), es ist also eine reine Entwicklungsabhängigkeit.

## 6. Tests — derzeit abgeschaltet

Auf diesem Branch laufen in CI **keine UI-Tests** (weder Vitest noch
Playwright), damit deine Commits nicht daran scheitern. Typecheck, Lint und
Build laufen weiter — ein Commit, der nicht kompiliert, ist auch ohne Tests
kaputt.

- `npm run test` lokal führt weiterhin die Tests der Datenschicht (`src/core`)
  aus; die sind grün und bleiben es, solange `core/` nicht geändert wird.
- `npm run e2e` meldet „0 tests“ und ist damit grün.
- `e2e/daemon.ts` (startet einen Daemon für Playwright) ist für deine späteren
  E2E-Tests schon da; die alten Specs liegen in `legacy/e2e/`.

**Wieder einschalten:** in `.github/workflows/ci.yml` und
`.github/workflows/release.yml` die Zeilen mit `if: false # UI-REDESIGN`
löschen (`grep -rn UI-REDESIGN .github`).
