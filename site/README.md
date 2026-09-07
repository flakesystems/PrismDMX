# `prism-site` — prismdmx.de, die Startseite

Die Seite, auf der jemand landet, der von PrismDMX gehört hat und noch nicht
weiß, was es ist. Ein Aufmacher, der Prozessschnitt als Argument, die
Funktionen, **vier Nachbauten der Oberfläche zum Anschauen**, der ehrliche
Stand der offenen Beta, die Zukunftspläne — bis hin zu eigener Hardware — und
der Download.

```bash
cargo run -p prism-site                                   # nach site/dist
cargo run -p prism-site -- --out /tmp/start
cargo run -p prism-site -- --docs http://localhost:8081   # zum Nebeneinanderlegen
```

## Warum das nicht `web/` ist

`prism-web` erzeugt **docs.prismdmx.de** und hat genau eine Eigenschaft, für die
es existiert: *es kann nicht von einem Release abweichen, weil es keinen eigenen
Inhalt hat.* Ein Test dort weist jedes `<script>` zurück, weil eine
Handbuchseite, die ein Skript braucht, auf dem Rechner im Rack nicht lesbar ist.

Eine Startseite ist die Umkehrung von beidem. Sie **ist** Inhalt, sie ist eine
Gestaltung und keine Übersetzung eines Dokuments, und sie soll zeigen, wie sich
die Software anfühlt — was ohne Bewegung nicht geht. Beides in einen Crate zu
legen hieße, `prism-web` seine Eigenschaft zu nehmen und seinen Test zu
entschärfen. Die Trennung ist billiger als die Ausnahmen.

|  | `prism-web` | `prism-site` |
|---|---|---|
| Domain | `docs.prismdmx.de` | `prismdmx.de` |
| Inhalt | aus `docs/`, keiner eigener | eine Vorlage, die diesem Crate gehört |
| JavaScript | keins, per Test | ja — `assets/site.js` begründet es |
| Abhängigkeiten | `pulldown-cmark`, `sha2` | **keine** |
| Getestet auf | jede Seite, jeder Anker, jede Prüfsumme | dass nichts Falsches draufsteht |

## Was von Hand geschrieben ist und was nicht

Die Seite selbst ist von Hand geschrieben — `assets/index.html`,
`assets/site.css`, `assets/site.js` —, weil eine Gestaltung nichts ist, das ein
Generator besser kann. Was **nicht** von Hand geschrieben wird, sind die vier
Dinge, die veralten, und die stehen als `{{…}}` in der Vorlage:

| Platzhalter | Woher |
|---|---|
| `{{version}}` | `[workspace.package] version` — dieselbe Zahl wie `prismd --version` |
| `{{installer}}` | daraus gebildet: `PrismDMX_<version>_x64-setup.exe` |
| `{{docs}}` | `DOCS`, mit `--docs` überschreibbar |
| `{{repository}}`, `{{releases}}` | Konstanten in `src/lib.rs` |

Ein Platzhalter, den niemand einsetzt, hält den Build an. Zwei geschweifte
Klammern in dunklem Fließtext sehen aus wie Gestaltung, und genau deshalb würde
sie sonst niemand finden.

## What it may not contain

**Keine Plattform-Codepfade und keinen zweiten Inhalt.** `#[cfg(target_os = …)]`
ist auf vier Crates beschränkt (`ARCHITECTURE_SPEC.md` §10.1) und dieser ist
keiner davon; eine Zeichenkette in ein Dokument einzusetzen ist auf jedem Ziel
dasselbe, und der ARM64-Cross-Check übersetzt diesen Crate mit.

**Keine Aussage, die nicht auch woanders im Repository steht.** Was die Seite
über den Funktionsumfang, den Stand der Beta und die Pläne sagt, kommt aus
`README.md`, `docs/RELEASE_NOTES.md` und `IMPLEMENTATION_PLAN.md`. Das ist keine
technische Regel, sondern die einzige, die eine Marketingseite ehrlich hält:
sie darf zuspitzen und sie darf gestalten, aber sie darf nichts versprechen, was
nicht anderswo mit seinen Einschränkungen aufgeschrieben ist. Der Abschnitt
*Noch nicht* ist deshalb genauso lang wie der Abschnitt *Läuft heute*.

**Keine Abhängigkeit im Browser.** Kein Framework, kein CDN, keine Schriftart
von einem fremden Server, nichts, was mitliest. Fünf Dateien, die der Browser so
nimmt, wie sie hier stehen.

**Kein zweites Firmenzeichen.** `assets/logo.png` und `assets/favicon.ico` sind
**Kopien von `ui/public/favicon.ico`** — desselben Bildes, das das Pult im
Fenster trägt und das der Installer als Programmsymbol mitgibt. Nachgezeichnet
wäre es ein zweites Zeichen: es sieht ähnlich aus, es weicht ab, und niemand
merkt, welches von beiden das richtige war.
`the_mark_on_the_page_is_the_one_the_desk_wears` vergleicht die Bytes, damit das
eine Tatsache über den Build bleibt und keine Absicht — und die Farben dieser
Seite sind aus dem Zeichen selbst abgelesen (`#e02883` nach `#8b3a8d`, das
Glyph `#6f2378`) und nicht danebengelegt.

## Testing it

```bash
cargo test -p prism-site
```

Die **Gestaltung ist ausdrücklich nicht getestet** — das ist eine Entscheidung
des Eigentümers, und sie ist richtig: diese Seite steuert kein Licht, und ein
Test über einen Farbverlauf wäre ein Test über einen Geschmack. `CLAUDE.md`s
Abdeckungsziele gelten für `engine/`, `programmer/` und `protocols/`, nicht
hierfür.

Was `tests/site.rs` prüft, ist die eine Sorte Fehler, die eine Startseite
trotzdem macht und die niemand sieht, bevor sie ausgeliefert ist: **dass etwas
Falsches draufsteht.** Ein stehen gebliebener Platzhalter, eine Version, die
nicht die des Builds ist, ein verlorener Link, eine Datei, die beim Kopieren
vergessen wurde.

## Deployment

Selbst gehostet, in einem Container — `deploy/` hat den Dockerfile, die
`docker-compose.yml` und den nginx-Vhost, der diese Seite unter `prismdmx.de`
und `prism-web`s Ausgabe unter `docs.prismdmx.de` ausliefert.
[`deploy/README.md`](../deploy/README.md) ist die Anleitung, einschließlich des
Wegs für einen LXC ohne Docker.

Ein Punkt daraus ist es wert, hier zu stehen, weil er eine Einschränkung des
Eigentümers ist und keine Vorliebe: **SSH auf diesen Server geht nur über VPN.**
Ein Build-Runner kann also nicht hineinschieben. Der Server **holt** — er baut
das Image selbst aus einem `git pull`, oder er zieht ein Release-Asset über
ausgehendes HTTPS. Kein eingehender Port, kein Deploy-Schlüssel bei GitHub.
`ARCHITECTURE_SPEC.md` und `.github/workflows/pages.yml` halten dieselbe
Überlegung für den Fall fest, dass jemand später doch `rsync` schreiben will.

## Sessions

Gebaut in **S55**, nach S41s Handbüchern und S42s Dokumentationsseite, aus deren
letztem offenem Abnahmekriterium: *ein Fremder kommt von der Startseite zu einem
laufenden Pult.* Bis dahin war die Startseite die Startseite der
**Dokumentation**, und ein Handbuchindex beantwortet die erste Frage eines
Fremden nicht, weil die erste Frage nicht *wo steht es* ist, sondern *was ist
das*.
