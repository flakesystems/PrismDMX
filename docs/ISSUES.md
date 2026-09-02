# ISSUES.md — was an der Oberfläche nicht stimmt

## Wie ein Eintrag aussieht

Eine Überschrift, die in einem Satz sagt, was falsch ist, und darunter fünf
Zeilen. Nicht alle fünf müssen ausgefüllt sein — **Was passiert** und **Was
passieren soll** sind die beiden, ohne die ein Eintrag nicht bearbeitbar ist.

```markdown
### B7 — Der Executor-Strip zeigt nach dem Laden einer Show die alten Namen

- **Wo:** Konsolen-Shell, Executor-Strip
- **Schwere:** ärgerlich
- **Was passiert:** Nach `Load Show` stehen die Beschriftungen der vorigen Show
  in den Feldern, bis man ein Fenster umschaltet.
- **Was passieren soll:** Die Beschriftungen gehören zur Show, also wechseln sie
  mit ihr.
- **So sieht man es:** Show A laden, Executor 3 benennen, Show B laden — Name
  von A steht noch da. Jedes Mal.
- **Ergebnis:** ☐ offen
```

**Die Nummern werden nie neu vergeben.** Sie sind Identität, nicht Reihenfolge —
dieselbe Regel wie bei den Session-Nummern in `IMPLEMENTATION_PLAN.md`. Ein
erledigter Eintrag bleibt mit seinem Ausgang stehen; er wird nicht gelöscht, und
seine Nummer wird nicht wiederverwendet. Die Reihenfolge innerhalb eines
Abschnitts ist egal, und Lücken in der Zählung sind kein Fehler.

## Der Ausgang eines Eintrags

Die Zeile **Ergebnis** füllt S43 aus, nicht der Eigentümer. Es gibt genau drei
Ausgänge, und „weggelassen" ist keiner davon:

| Zeichen | Bedeutung |
|---|---|
| ☐ | **offen** — der Zustand, in dem jeder Eintrag geschrieben wird |
| ✅ | **behoben** — mit einem Satz, was geändert wurde, und dem Test, der es festhält |
| ⛔ | **abgelehnt** — mit dem Grund. Ein Eintrag darf abgelehnt werden; er darf nur nicht kommentarlos verschwinden |
| ➡️ | **wurde S45** (oder eine andere Nummer) — zu groß für einen Nachmittag, also eine eigene Session im Plan, mit Deliverables und Exit-Kriterien |

## Die Schwere

Sie steuert die Reihenfolge, nicht die Berechtigung — auch ein Schönheitsfehler
wird behoben oder begründet abgelehnt.

| Wort | Bedeutung |
|---|---|
| **blocker** | Damit kann man kein Pre-Release herausgeben. Etwas geht kaputt, verliert Daten, oder ein Operator kommt an eine Funktion nicht heran |
| **ärgerlich** | Man kann damit arbeiten, aber es kostet jedes Mal Aufmerksamkeit oder einen Umweg |
| **Schönheitsfehler** | Es sieht falsch aus oder liest sich falsch, aber es hält niemanden auf |

## Wenn du unsicher bist, ob etwas hierhergehört

Rein damit. Ein Eintrag, der sich als „so gedacht" herausstellt, kostet eine
Zeile Begründung; ein Eintrag, den niemand aufgeschrieben hat, kostet den ersten
Abend, an dem jemand anderes das Pult bedient.

---

# Die Einträge

Die Abschnitte sind nur zum Wiederfinden da. Passt etwas in keinen, kommt es
unter **Sonstiges** — die Abschnitte umzusortieren ist Arbeit für später und
nicht für den Moment, in dem man einen Fehler bemerkt hat.

## Canvas, Fenster und Layout

*Alles, was mit der Anordnung auf dem Bildschirm zu tun hat: Fenster öffnen,
schließen, anordnen, Ansichten, Seiten, was zu groß oder zu klein ist.*

<!-- Erster Eintrag hier. Vorlage:

### B1 — <ein Satz, was falsch ist>

- **Wo:**
- **Schwere:**
- **Was passiert:**
- **Was passieren soll:**
- **So sieht man es:**
- **Ergebnis:** ☐ offen

-->

### B5 — Manche Notifications lassen sich nicht schließen

- **Wo:** Notifications
- **Schwere:** blocker
- **Was passiert:** Es gibt Notifications, die keinen Schließen (X) Knopf haben
- **Was passieren soll:** Alle Notifications sollten geschlossen werden können
- **So sieht man es:** Ein Beispiel für eine nich schließbare Notification ist das erfolgreiche Exportieren einer JSON. Fehler lassen sich schließen
- **Ergebnis:** ✅ **behoben — im zweiten Anlauf, die erste Diagnose war falsch.** Ich hatte geschrieben, der Knopf sei nur schlecht sichtbar. Der Eigentümer hat widersprochen, und im Browser nachgemessen hatte er recht: der Knopf saß **ein Pixel außerhalb** von `.notice-wrapper`, und `overflow: hidden` hat alle 28 px davon abgeschnitten. Zwei Fehler, und beide treffen genau die Meldungen, die der Eintrag nennt. **Erstens das Layout:** die automatische Mindestbreite eines Flex-Items ist seine min-content-Breite, und die Meldung eines erfolgreichen Exports ist ein absoluter Pfad **ohne Leerzeichen**. Also weigerte sich der Text zu schrumpfen und schob den Knopf aus der Box. Eine Fehlermeldung ist ein Satz, sie bricht um, ihr Knopf überlebt — exakt das *Fehler lassen sich schließen, das hier nicht* aus dem Eintrag. `min-width: 0` und `overflow-wrap: anywhere` auf den Text, `flex-shrink: 0` auf den Knopf. **Zweitens das Wegräumen:** es hängte allein an `transitionend`, also blieb eine Meldung in einem Browser, der die Animation nicht läuft, für immer stehen — mit einem Knopf, der beim Drücken nichts tut. Jetzt liegt eine Frist dahinter, und die **liest die Dauer vom Element ab** statt 300 ms ein zweites Mal hinzuschreiben; `prefers-reduced-motion` schaltet die Dauer auf null und die Meldung geht sofort. Test: *the close button of a long notice is inside the notice* (`ui/e2e/settings.spec.ts`) — **im Browser**, weil `jsdom` kein Layout rechnet und deshalb jeder Unit-Test hier grün war, während der Knopf unerreichbar war. Genau das ist der Grund, warum dieser Test dort steht.


### B8 — DMX Werte Anzeige im Fixture Sheet verbuggt

- **Wo:** Fixture Sheet Fenster
- **Schwere:** ärgerlich
- **Was passiert:** Das Scaling der DMX Werte Anzeige ist verbuggt
- **Was passieren soll:** Die Anzeige soll komplett entfernt werden, da sie unnötig ist. Die Tabellarische Anzeige reicht.
- **So sieht man es:** Fixture Sheet Fenster öffnen
- **Ergebnis:** ✅ **behoben** — die Canvas-Spalte mit den Live-Balken ist aus dem Fixture Sheet raus. Was auf dem Kabel liegt, zeigt das `DMX Sheet`-Fenster, Kanal für Kanal und ohne Fixtures dazwischen. `patch/live.ts` bleibt liegen, mit seinen Tests: es ist das Muster für jede spätere Ansicht mit Live-Werten pro Zeile (S27), und was entfernt wurde, ist die *Benutzung* durch dieses Sheet. Test: `ui/src/patch/sheet.test.tsx`.


### B9 — Add Window verschönern

- **Wo:** Canvas
- **Schwere:** Schönheitsfehler
- **Was passiert:** Das hinzufügen von Fenstern durch ein Dropdown ist hässlich
- **Was passieren soll:** Das Dropdown soll komplett entfernt werden und durch ein Modal ersetzt werden, dass sich öffnet, wenn man einen Rechtsklick auf eine leere Stelle im Canvas macht
- **So sieht man es:** In der Hauptansicht
- **Ergebnis:** ✅ **behoben** — das Dropdown ist weg, nicht versteckt. Ein Rechtsklick auf eine leere Stelle des Canvas, die Insert-Taste oder eine F-Taste am Pult öffnen `canvas/picker.tsx`. Der Zustand *Wähler offen* ist `Session::windowPicker` und damit Sitzungszustand — das X-Touch öffnet denselben Wähler, den der Bildschirm zeigt. Tests: `has no window picker in it at all` (`ui/src/canvas/viewbar.test.tsx`) und `every_session_command_applies_and_emits_a_session_patch` (`crates/prism-core/tests/session_commands.rs`).


### B10 — Fenster dürfen sich nicht überlagern

- **Wo:** Canvas
- **Schwere:** Schönheitsfehler
- **Was passiert:** Wenn man mehrere Fenster öffnet, legen sie sich übereinander
- **Was passieren soll:** Neu Fenster suchen sich einen freien Platz auf dem Canvas und lassen sich beim Verschieben auch nicht überlagern
- **So sieht man es:** Mehrere Fenster im Canvas öffnen
- **Ergebnis:** ✅ **behoben** — `prism_core::layout` sucht einem neuen Fenster einen freien Platz (Ecken der schon liegenden Fenster, drei Verkleinerungsstufen, dann eine Absage) und lässt ein gezogenes nur dort landen, wo es **niemanden neu** verdeckt. Die Regel ist *Überlappung darf nur schrumpfen*, damit ein Fenster, das schon unter einem anderen liegt, noch herausgezogen werden kann. Der Daemon platziert, nicht der Client (**D11**). Tests: elf in `crates/prism-core/src/layout.rs`, darunter `six_default_windows_fill_the_canvas_and_the_seventh_is_refused`, `a_move_that_buries_a_neighbour_is_refused` und `a_window_already_buried_can_still_be_dragged_out`.

  **Nachtrag (Handprobe, 2026-08-27):** Ablehnen war richtig und reichte nicht. Der Operator sah das Fenster dem Zeiger über den Nachbarn *folgen* und dann zurückspringen, wenn die Absage ankam — zwei Fenster bündig nebeneinander zu setzen hieß also, auf eine Position zu zielen, die man sich nicht ansehen kann. Die Kanten des Canvas machten das nie, weil `movedBy` dorthin *klemmt*. Jetzt sind die Nachbarn auch Wände: `geometry.ts::slidTo` und `grownTo` lösen die Bewegung achsenweise auf, sodass ein Fenster an dem entlangrutscht, wogegen es drückt, statt an der ersten Ecke stehenzubleiben. Das ist eine **Vorhersage**, keine zweite Meinung: die Position bleibt die der Session, jeder Zug geht weiter als `PlaceWindow` hinaus, und `may_place`s Regel *Überlappung darf nur schrumpfen* gilt hier genauso — ein schon verdecktes Fenster wird von niemandem geblockt. Tests: fünf in `ui/src/canvas/drag.test.ts`, darunter *stops where it meets one instead of crossing it and being pulled back* und *lets a window that is already buried be dragged out from under*.


### B11 — Neue Views sollten leer sein

- **Wo:** Views/Canvas
- **Schwere:** ärgerlich
- **Was passiert:** Wenn man einen neuen View erstellt, enthält er alle Fenster des vorherigen Views
- **Was passieren soll:** Der Canvas sollte sich leeren, wenn man einen neuen View erstellt
- **So sieht man es:** Von einem View mit Fenstern aus einen neuen View erstellen
- **Ergebnis:** ✅ **behoben** — *Neu* und *Speichern* sind zwei Befehle geworden. `Command::NewView` legt eine leere Ansicht an, wählt sie aus und leert den Canvas; vorher schickte der Knopf `StoreView`, weshalb die neue Ansicht die Fenster der alten mitbrachte. Auch über die Zeile erreichbar: `New View 3 "Front"`. Tests: `makes an empty view rather than storing the canvas as one` (`ui/src/canvas/viewbar.test.tsx`) und `every_session_command_applies_and_emits_a_session_patch`.

## Kommandozeile

*Die Zeile selbst, ihre Rückmeldungen, ihre Fehlermeldungen, die Historie, und
alles, was sie annehmen oder ablehnen sollte und nicht tut.*

### B12 — Buttons sollten in ein Fenster verschoben werden

- **Wo:** Konsolen Buttons
- **Schwere:** Schönheitsfehler
- **Was passiert:** Buttons werden unter der Konsole angezeigt, stören da aber
- **Was passieren soll:** Es soll ein neues Fenster geben, in dem die Buttons bedient werden können. Ansonsten soll man nur tippen oder mit den Buttons auf dem Surface arbeiten
- **So sieht man es:** Hauptansicht
- **Ergebnis:** ✅ **behoben** — die Konsolen-Tasten sind das Fenster `Command Keys` (`ui/src/desk/keypad.tsx`). Keine davon sendet einen eigenen Befehl: `run` schreibt sein Wort und schickt es ab, `write` schreibt es und wartet, `append` hängt es an — den Rest entscheidet die Zeile, genau wie bei einer getippten. Und jede Taste gibt die Tastatur an die Zeile zurück, weil `Store ` und `Cue ` auf eine Zahl warten. Test: `ui/src/desk/desk.test.tsx`.


### B13 — Vorschlagsanzeige entfernen

- **Wo:** Unter der Konsole
- **Schwere:** Schönheitsfehler
- **Was passiert:** Unter der Konsole werden Vorschläge angezeigt
- **Was passieren soll:** Die Vorschläge sollen nicht mehr angezeigt werden, Tab Complete soll bleiben
- **So sieht man es:** Hauptansicht
- **Ergebnis:** ✅ **behoben** — die Vorschlagsleiste unter der Zeile ist weg, `completions()` und Tab-Complete sind geblieben. Test: `ui/src/desk/shell.test.tsx`.


### B14 — Vorschläge sollten groß  geschrieben sein

- **Wo:** Tab Complete
- **Schwere:** Schönheitsfehler
- **Was passiert:** Alle Tab Complete Vorschläge sind klein geschrieben
- **Was passieren soll:** Sie sollten stattdessen mit einem Großbuchstaben beginnen, um konsistent mit Schreibweisen an anderen Stellen zu beiben
- **So sieht man es:** Tab Complete Vorschläge
- **Ergebnis:** ✅ **behoben** — `completions()` gibt die Wörter groß geschrieben zurück, so wie sie überall sonst im Pult stehen. Die Zeile selbst liest weiter unabhängig von der Schreibweise. Test: `ui/src/desk/console.test.ts`.

## Executors und Wiedergabe

*Der Strip, die Fader, Go / Pause / Off, die Sequenzen, was während des Laufens
angezeigt wird.*

### B15 — Executor Aktionen können nicht bearbeitet werden

- **Wo:** Executor Strip
- **Schwere:** blocker
- **Was passiert:** Aktionen wie Go+, Go-, Flash, On und Off, sowie Master/XFade können nicht bearbeitet werden
- **Was passieren soll:** Das UI Redesign sieht vor, den Execuor Strip zu entfernen und in ein Fenster zu bewegen. Dort soll dann auch eingestellt werden können, welche Aktionen von den Buttons und dem Fader ausgeführt werden sollen
- **So sieht man es:** Es passiert nichts wenn man einen Executor rechtsklickt
- **Ergebnis:** ✅ **behoben in S45** — der Editor sitzt unter dem Strip im `Executors`-Fenster: Fader, Encoder und die vier Tasten des **ausgewählten** Executors, jede als Liste aus `prism_domain` erzeugt, dazu die **eigene Zeile** *diese Kommandozeile senden* (`ExecutorButtonFunction::CommandLine`) mit einem Feld, in das der Operator die Zeile schreibt. Der Editor **schickt kein Kommando**, er schreibt eine Zeile und sendet sie — `Assign Executor 1 Fader Master`, `Assign Executor 1 Button 2 Go+` — womit Fenster, Zeile und gebundene X-Touch-Taste ein Weg sind statt drei. Ob das ein `Command` oder ein `MachineChange` ist, war die Entscheidung dieser Session: ein `Command`, und `docs/IPC_PROTOCOL.md` §5 trägt den Grund. Die untere Zeile des Scribble Strips zeigt jetzt, was die fünf Bedienelemente tun, statt der Prozentzahl, die der Motorfader darunter ohnehin schon zeigt (`docs/MCU_MAPPING.md` §4.1). Tests: `ui/src/desk/executoreditor.test.tsx` (welche Zeile jeder Wähler schreibt), `ui/src/desk/console.test.ts` (zu welchem Kommando die Zeile wird), `crates/prism-surface/tests/bindings.rs` (die gebundene Taste trägt dieselbe Zeile), `ui/e2e/executors.spec.ts` (im Browser, gegen einen echten Daemon), `crates/prismd/src/core.rs` (die eigene Zeile überlebt Speichern und Neustart).


### B18 — Mehrere Executors einer Sequence sind nicht syncronisiert

- **Wo:** Executor Strip
- **Schwere:** ärgerlich
- **Was passiert:** Wenn eine Sequence mehrere Executor hat, die die gleichen Button/Fader Tpyen haben, kann man diese unabhängig von einander bewegen
- **Was passieren soll:** Jede Sequence sollte nur einen Faderwert pro Fadertyp und einen Buttonwert pro Button Typ haben können. So sollten sich zwei Executor mit Master Fadern, die zu der gleiche Sequnce gehören syncron verhalten. Ist ein Fader XFade und ein Fader Master, sollten beide unabhängig voneinander funktionieren
- **So sieht man es:** Einer Sequence mehrere Executor zuweisen
- **Ergebnis:** ✅ **behoben in S45** — der Zustand liegt jetzt dort, wo eine Sequence nur einen davon haben kann. `PlaybackId` ist die Nummer der Cue-Liste, `Show::playback_of` löst einen Executor auf die Liste auf, die auf ihm steht, und `MergeBody` bekommt **ein** Playback pro Liste. Masterlevel, Rate, „läuft" und Cue-Zeiger sind vom `Executor` auf die `Sequence` gewandert; ein Executor ist ein **Griff**: welche Liste, was der Fader tut, was der Encoder tut, was die vier Tasten tun. Zwei Master-Fader auf einer Liste sind damit zwei Griffe an einer Zahl, ein Master und ein XFade bleiben unabhängig, und `Go` auf einem von beiden bewegt **einen** Cue-Zeiger. Auf Frames zugesichert: `two_master_faders_on_one_cue_list_move_one_light`, `a_master_and_a_crossfade_on_one_cue_list_are_two_handles` und `a_go_on_either_handle_advances_one_cue_pointer` in `crates/prismd/src/core.rs`, plus **B18** in `ui/e2e/executors.spec.ts` im Browser. Eine `.prism`-Datei von vorher öffnet unverändert, und das Level, das ihr Executor trug, wird auf die Liste übernommen (`ShowStore::carry_levels_onto_their_cue_lists`).


### B36 — Crossfade falsch implementiert

- **Wo:** Executor
- **Schwere:** ärgerlich
- **Was passiert:** Im Crossfade Modus wird ein Fadeover in einer Faderbewegung gemacht. Danach fährt er zurück auf 0 für den nächsten Fade. Das ist unpraktisch.
- **Was passieren soll:** Es soll 2 Crossfadeade Modi geben: "Fade" und XFade": "Fade" soll beim hochfaden den aktuellen Cue ausfaden und beim runterfaden den nächsten Cue einfaden. "XFade" soll beim hochfaden zwischen dem aktuellen und dem nächsten crossfaden und beim runterfaden zwischen dem nächsten und übernächsten crossfaden, so dass man durch immer wieder hoch und runter faden durch die cuelist gehen kann. In keinem Fall soll der Fader nach einer Bewegung irgendwie zurück bewegt werden. Wenn der Fader in der Mitte anhält soll der Crossfade an dieser Stelle gehalten werden.
- **So sieht man es:** Executor auf Crossfade stellen
- **Ergebnis:** ☐ offen


## Programmer, Presets, Groups

*Auswählen, Werte setzen, Speichern, Clear, die Pools.*

### B1 — Farbwerte im Programmer sollten auf 100% starten

- **Wo:** Programmer
- **Schwere:** ärgerlich
- **Was passiert:** Wenn man ein Fixture anwählt sind die Farbwerte standardmäßig auf 0%
- **Was passieren soll:** Die Farbwerte sollten auf 100% sein. Zum Farben mixen dreht man sie runter
- **So sieht man es:** Fixture im Programmer anwählen und in die Farb Sektion gehen
- **Ergebnis:** ✅ **behoben** — `library::colour()` gibt jedem Color-Attribut `u16::MAX` als Ruhewert, und der OFL-Import tut dasselbe für jedes Profil, das keinen eigenen Default mitbringt. Ein Fixture kommt offen aus der Auswahl, und Farbe mischt man von oben herunter. **Zweiter Nachtrag (2026-08-28): der Eintrag war noch offen, und die Zeile oben stimmte nicht.** Der Eigentümer meldete die Farben weiterhin auf 0 %. Ursache: die generischen Profile waren richtig, der **OFL-Konverter** aber glaubte einem im Datei angegebenen `defaultValue` — mit dem Argument, das sei der Hersteller, der sagt, wo der Kanal ruht. In der mitgelieferten Bibliothek nachgezählt: **391 von 1131 Farbkanälen geben einen Default an, und 387 davon geben Null an.** Auf den meisten echten Fixtures war die Nachgiebigkeit also der ganze Fehler. Die beiden beantworten verschiedene Fragen: der `defaultValue` des Herstellers sagt, wo die *Lampe* ohne DMX steht — dunkel, die einzige sichere Antwort einer Lampe — und wo ein *Pult* einen Farbkanal parkt, ist eine Konvention des Pults. Ein Farbkanal ruht jetzt offen, **egal was die Datei sagt**. Nebenbei gefunden: `as_u64` antwortet `None` auf `"50%"`, also fiel ein in Prozentform angegebener Default stillschweigend auf Null — wird jetzt gelesen. Und die Behauptung, `patch_to_frame.rs` weise denselben Wert am DMX-Ausgang nach, war schlicht falsch: **diesen Test gab es nicht.** Genau deshalb hat die zweite Hälfte des Fehlers überlebt. Tests, jetzt vorhanden: `a_colour_channel_rests_open_and_nothing_else_does` (`crates/prism-core/src/library/mod.rs`), `no_profile_in_the_installed_library_rests_a_colour_shut` (`crates/prism-core/tests/fixture_library.rs`, über den echten Baum aus 635 Dateien), `a_default_value_written_as_a_percentage_is_read_as_one` (`ofl.rs`) und `a_colour_channel_rests_open_at_the_wire` (`crates/prism-engine/tests/patch_to_frame.rs`) — die zweite Hälfte der Frage aus Runde 2, diesmal wirklich. Von Hand nachgesehen: ADJ Dekker LED 8ch aus der Bibliothek gepatcht, Fixture gewählt, Color-Bank liest Red/Green/Blue/White auf 100 %, Dimmer auf 0 %.

  **Dritter Nachtrag (2026-08-28): der Eintrag war *immer noch* offen, und die Ursache lag diesmal im Patch-Fenster.** Der Eigentümer meldete die Farben zum dritten Mal auf 0 %. Der Daemon war korrekt gebaut und die Bibliothek stimmte — aber eine Show **bettet ihre Profile ein** (S11), und meine B19-Implementierung schickte `EmbedFixtureType` nur, *wenn die Show den Schlüssel noch nicht trug*. Eine Show, die vor dem Bibliotheksfix gepatcht wurde, behielt ihre alte Kopie also **für immer**: das Profil aus der Liste zu wählen schickte nichts, und keine andere Geste in der Oberfläche konnte die Kopie ersetzen. *Neu patchen* — was ich zweimal als Lösung genannt hatte — tat schlicht nichts. Ein Profil aus der Bibliothek zu wählen heißt jetzt *nimm die Kopie der Bibliothek*, **jedes Mal**; der Daemon lehnt den Ersatz ab, wenn er einen stehenden Patch brechen würde. Die Zeile in der Liste sagt es auch: *in this show; picking re-reads it*. Und der Test, der das falsche Verhalten festhielt, war meiner — er sagte *was nicht zweimal passieren darf, ist das Einbetten*, und genau das war die Falle. Tests: `re-reads a profile the show already carries, rather than keeping the old copy` (`ui/src/patch/patchwindow.test.tsx`) und `embedding_a_profile_again_replaces_the_copy_the_show_was_carrying` (`crates/prism-core/src/show.rs`) — die zweite Hälfte der Kette, die vorher niemand geprüft hatte. **Eine bestehende Show braucht dafür eine Wahl pro Profil, nicht pro Fixture**: die Kopie ist geteilt, also erneuert ein einziges Neu-Wählen sie für alle Lampen dieses Typs.

  **Nachtrag (Handprobe, 2026-08-27):** die Profile ruhten offen und der Programmer zeigte trotzdem einen Strich — also suchte man den Wert weiter unten und schob ihn hoch. Der Strich stand auf dem Grundsatz *absent is not zero*: der Programmer ist dünn besetzt, Abwesenheit heißt *die Playbacks entscheiden*, und 0 % hätte *schwarz* gesagt. Der Grundsatz stimmt weiter; falsch war, ihn von der **Zahl** tragen zu lassen. Jetzt zeigt ein Encoder den Wert des Programmers, wenn er einen hält, und sonst den **Ruhewert aus dem Profil** — eine Farbe liest also 100 % und wird heruntergezogen. Was den alten Grundsatz rettet, ist die Markierung aus B21: *überschreibend* ist ein eigenes Zeichen geworden, kein fehlender Wert. **Eine Show, die vor B1 gepatcht wurde, trägt ihre eigenen alten Profile** (S11: eine Show bettet ein, sie referenziert nicht) und liest deshalb weiter 0 % — neu patchen ist, was sie bewegt. Tests: *reads the resting value when the programmer holds nothing, and says it is not overriding* und *reads a colour at full, because that is where a colour rests* (`ui/src/desk/programmer.test.ts`).


### B2 — Clear sollte immer anzeigen was gecleared werden kann

- **Wo:** Programmer
- **Schwere:** ärgerlich
- **Was passiert:** Clear kann jederzeit durch alle clear Stufen cyclen. So kann der clear button auf manchen clear Stufen stehenbleiben, obwohl danach wieder etwas verändert wurde. Das ist unübersichtlich
- **Was passieren soll:** Stattdessen sollte der clear button wenn es etwas zu clearen gibt die Farbe zur jeweiligen clear Stufe ändern und außerdem nicht die Stufe wechseln, wenn es nichts zu clearen gibt.
- **So sieht man es:** Clear Button mehrmals drücken
- **Ergebnis:** ✅ **behoben** — die Clear-Stufe wird nicht mehr gezählt, sondern aus dem Programmer *abgeleitet* (`ProgrammerState::stage()`), also kann sie nicht mehr veralten. Vier Stufen statt drei: `Nothing` ist die, bei der der Knopf abgeschaltet ist und seine Farbe verliert, die anderen drei färben ihn nach dem, was der nächste Druck wegnimmt. Tests: `clearing_an_empty_programmer_does_nothing_and_says_so` und `the_stage_follows_the_contents_after_any_other_interaction` (`crates/prism-core/tests/programmer.rs`), `reads every stage the Clear key has, and refuses one it has not` (`ui/src/ipc/protocol.test.ts`).


### B7 — Teile des Programmers sind nicht sichtabr (sollte sowieso durch das UI Redesign behoben werden)

- **Wo:** Programmer
- **Schwere:** blocker
- **Was passiert:** Einige Auswahlfelder im Programmer overflowen über den Seitenrand und sind abgeschnitten
- **Was passieren soll:** Es sollte nichts abgeschnitten sein
- **So sieht man es:** Im Programmer in der Menüleiste, sowie im Farben und Beam Menü
- **Ergebnis:** ✅ **behoben** — durch das Redesign, wie der Eintrag selbst vermutet. Der Programmer ist jetzt ein Band mit fester Höhe: sieben Bank-Tasten in zwei Spalten, ein Seitensteller, **vier Encoder in einem festen Raster** (die nicht mehr strecken, wenn nicht alle Plätze besetzt sind) und die gewählte Sequence. Nichts davon wächst über den Rand, weil nichts davon in der Breite von seinem Inhalt abhängt — eine Bank mit mehr Parametern als auf eine Seite passt wird geblättert, nicht breiter. Die Menüs, die vorher abgeschnitten waren, gibt es nicht mehr: das Fenster-Dropdown ist der Wähler (B9), die Konsolentasten sind ein Fenster (B12), die Vorschlagsleiste ist weg (B13), und die Ablesungen sind das `Status`-Fenster. Was hier steht, ist Kopfzeile, Canvas, Kommandozeile, Programmer — und das ist die Skizze des Eigentümers. Test: *the screen is a device screen at 1280 x 720 / at 4K, with every window open* (`ui/e2e/desk.spec.ts`), das **jeden** Fenstertyp aus dem Wähler öffnet — aus der generierten Liste, nicht aus einer hier abgeschriebenen — und dann Seite, Canvas, Programmerband, Kommandozeile und **jeden Fensterrahmen** auf Null prüft. Ein Browser-Test, weil `jsdom` kein Layout rechnet und ein Überlauf dort unsichtbar ist.


### B16 — Fixtutures, Gruppen und Sequences sind nicht auswählbar

- **Wo:** Fixture Sheet, Group Sheet und Sequence Sheet
- **Schwere:** blocker
- **Was passiert:** Fixtures, Gruppen und Sequences sind nicht durch einen Klick auf ein Tabellenelement auswählbar
- **Was passieren soll:** Fixtures, Gruppen und Sequences sollten durch einen Klick auf ein Fixture, eine Gruppe oder eine Sequence in ihren jeweiligen Sheets ausgewählt werden können
- **So sieht man es:** im Fixture, Group und Sequence Sheet auf ein Element klicken
- **Ergebnis:** ✅ **behoben** — in allen drei Sheets ist die **ganze Zeile** das Ziel, nicht die Zahl in der ersten Zelle, und sie ist auch mit Tab und Enter erreichbar (ein Pult wird im Dunkeln bedient). Der Klick schickt eine Kommandozeile, keinen eigenen Befehl — was die Zeile kann, kann der Klick. Tests: `ui/src/patch/sheet.test.tsx`, `ui/src/show/grouppool.test.tsx`, `ui/src/show/sequencesheet.test.tsx`.


### B17 — Auswahl stackt sich nicht

- **Wo:** Fixture Auswahl
- **Schwere:** blocker
- **Was passiert:** Wenn ein Fixture ausgewählt ist und ein anderes gewählt wird, wird die Auswahl ersetzt
- **Was passieren soll:** Das Fixture sollte stattdessen zusätzlich ausgewählt werden. Die Auswahlt soll sich also stacken, bis gecleared wird
- **So sieht man es:** Mehrere Fixtures nacheinander auswählen
- **Ergebnis:** ✅ **behoben** — ein Klick schickt `+ Fixture 3`, also `SelectFixtures { mode: Toggle }`: die Auswahl stapelt sich, und dieselbe Geste nimmt ein Fixture wieder heraus. Das `+` gibt es auch getippt, für Fixtures und für Gruppen. Weggenommen wird die Auswahl nur mit Clear — die Antwort des Eigentümers aus Runde 2. Tests: `ui/src/desk/console.test.ts` und `ui/src/patch/sheet.test.tsx`.


### B21 — Es ist nicht klar, welche Attribute überschrieben werden

- **Wo:** Programmer
- **Schwere:** blocker
- **Was passiert:** Man sieht einem Attribut nicht an, ob es gerade überschreibt oder nur ruht
- **Was passieren soll:** Es muss klarer markiert sein. Ein Rechtsklick auf eine Kategorie wählt alle ihre Attribute als überschreibend aus; ein Klick auf ein Attribut ohne es zu bewegen behält den Wert, überschreibt aber. (Überschreiben heißt: an dieser Stelle wird definitiv dieser Wert gesendet, egal was in einem vorherigen Cue passiert ist)
- **So sieht man es:** Im Programmer, mit einer laufenden Sequence dahinter
- **Ergebnis:** ✅ **behoben** — nachgereicht vom Eigentümer in der Handprobe am 2026-08-27, und es ist die Hälfte, die B1 erst sicher macht: weil beide Encoder jetzt eine Zahl zeigen, muss *ist das meins?* etwas anderes tragen als die Zahl. Ein überschreibender Encoder ist heller, hat einen Balken an der rechten Kante und fette Ziffern — drei Signale, jedes einzeln ausreichend, weil ein Pult in zwei Metern Entfernung im Dunkeln gelesen wird und nicht alle Farben gleich sehen (`CLAUDE.md`). **Beide Gesten sind ein Zug um null**: ein relatives `SetAttribute` startet bei dem, was der Programmer hält, oder beim Ruhewert, wenn er nichts hält — ein Delta von 0 schreibt also genau den Wert, der ohnehin schon hinausging, und ihn zu *schreiben* ist das Überschreiben. Kein neuer Command, und nichts im Frontend rechnet den Wert aus: `prism_core::Programmer::set_attribute` tut das, pro Fixture. Nie zweimal auf dasselbe Attribut, sonst verlöre ein Wert aus einem Preset stillschweigend seinen Link (S13s `presetRef`) durch einen Klick, der nichts ändern sollte. Tests: acht in `ui/src/desk/programmerband.test.tsx`. **Die andere Hälfte ist S48**: was der Programmer als überschreibend markiert, ist genau das, was ein Cue behaupten muss, damit ein Sprung dorthin zweimal dasselbe ergibt. — **Nachgetragen 2026-08-30 (S48): die andere Hälfte ist da.** Ein `CuePart` sagt jetzt, ob sein Wert weiterläuft (`Track`) oder am Ende des Cues zurückgenommen wird (`CueOnly`); *erbt* bleibt, was es war — gar kein Part. Der Cue Viewer zeichnet die drei in derselben Sprache wie die Encoder: eine Behauptung ist heller, fett und hat den Balken an der rechten Kante, ein geerbter Wert ruht in Klammern, und ein Cue-only-Wert trägt zusätzlich einen Punkt, weil er *während* des Cues genauso hinausgeht. Der geerbte Wert ist eine **Zahl** und kein Strich mehr, und das ist die Antwort auf die Frage, mit der man dieses Fenster öffnet: *was ändert Cue 7, und was lässt er stehen?* Ausgerechnet wird sie im Daemon (`Query::CueTracking`) und nirgends sonst — eine zweite Implementierung derselben Faltung im Frontend wäre eine zweite Meinung zu genau der Regel, die S48 auf **eine** Antwort bringt. Tests: die sieben in `describe("what a cue inherits")` (`ui/src/show/sequencesheet.test.tsx`) und der Playwright-Lauf `a cue sheet says what a cue asserts and what it inherits from the cues above`.


### B37 — Clear-Stufen in falscher Reihenfolge (GitHub #1)

- **Wo:** Programmer, Clear-Button
- **Schwere:** ärgerlich
- **Was passiert:** Der Clear-Button löscht beim ersten Druck die Ausgabe und erst beim zweiten Druck die Fixture-Auswahl
- **Was passieren soll:** Die Reihenfolge soll umgekehrt sein: erst Fixture-Auswahl clearen, dann die Ausgabe — andernfalls ist es nicht möglich, mehrere verschiedene Fixtures gleichzeitig zu programmieren
- **So sieht man es:** Mehrere Fixtures nacheinander programmieren und dann Clear drücken
- **Ergebnis:** ☐ offen


### B38 — Fehlende Kanal-Mappings für OFL-Fixtures (GitHub #2)

- **Wo:** Programmer, Fixture-Library (OFL)
- **Schwere:** ärgerlich
- **Was passiert:** Nicht alle Kanäle von Open-Fixture-Library-Profilen sind einem Attribut und einer Kategorie im Programmer zugeordnet. Außerdem fehlt die Unterstützung von OFL-Capabilities vollständig
- **Was passieren soll:** Alle relevanten OFL-Kanäle sollen auf Attribute und Kategorien gemappt werden; OFL-Capabilities sollen ausgewertet und unterstützt werden
- **So sieht man es:** Ein OFL-Fixture patchen und im Programmer aufrufen — einzelne Kanäle erscheinen dort nicht oder landen in der falschen Bank
- **Ergebnis:** ☐ offen


### B20 — Jog Wheel zu langsam

- **Wo:** Jog Wheel
- **Schwere:** ärgerlich
- **Was passiert:** Eine volle Umdrehung des Jog Wheel ändert den DMX Wert um ca. 1%
- **Was passieren soll:** Die Änderungsrate soll mit der Geschwindigkeit der Jog Wheel Drehung skalieren
- **So sieht man es:** Jog Wheel drehen
- **Ergebnis:** ✅ **behoben** — und es war keine fehlende Beschleunigung, sondern die falsche **Einheit**. Beide Kurven in `prism_surface::accel` waren in abstrakten *Schritten* geschrieben, die niemand weiter unten multipliziert hat: ein Schritt war ein 65 535stel, also bewegte ein sorgfältiger V-Pot-Klick einen Parameter um 0,0015 % und eine volle Radumdrehung um die gemessene 1 %. Neu sind beide Tabellen in Attributeinheiten geschrieben, mit `COARSE` = 257 (ein DMX-Schritt eines 8-Bit-Kanals) als Maß. Die Radzeilen sind genau das Zwanzigfache der gemessenen — die beiden Zahlen des Eigentümers, 1 % gemessen und 20 % gewünscht, reichen dafür aus, ohne irgendwo Rasten pro Umdrehung zählen zu müssen. Die Spreizung (achtfach zwischen Klick und Spin) bleibt, weil sie nie das Problem war. Tests: `every_jog_row_is_twenty_times_the_row_the_owner_measured`, `the_wheel_still_answers_eightfold_between_a_click_and_a_spin` und `one_detent_is_a_move_that_can_be_seen_on_both_curves` (`crates/prism-surface/src/accel.rs`). **Die Kalibrierung selbst gehört in die Handprobe**: kein Test darf das Gerät anfassen, also hält der Test das Verhältnis und die Hand das Gefühl.

## Patch und Fixture Sheet

*Den Rig aufbauen, Fixtures anlegen, adressieren, die Bibliothek.*

### B6 — ArtNet zeigt Health OK ohne angeschlossene Node

- **Wo:** Settings Outputs
- **Schwere:** ärgerlich
- **Was passiert:** ArtNet Nodes werden als Health OK angezeigt, obwohl sie nich angeschlossen sind
- **Was passieren soll:** Es sollte nur Health OK gezeigt werden, wenn tatsächlich eine Verbindung besteht
- **So sieht man es:** Settings Outputs mit einer konfigurierten ArtNet Node öffnen, ohne tatsächlich eine ArtNet Node anzuschlißen
- **Ergebnis:** ✅ **behoben in S46** — es gibt jetzt einen **Empfangsweg**, und damit eine Antwort statt einer Vermutung. `prism_protocols::artpoll` schreibt `ArtPoll` und liest `ArtPollReply` (Art-Net 4 §6): Kurz- und Langname, IP und MAC, die Port-Adress-Tabelle, die Statusbytes und die Firmware-Revision. `NodeDiscovery` fragt jede konfigurierte Node alle drei Sekunden und merkt sich, wer antwortet; der Daemon **faltet das in die gemeldete Health**, also liest ein Art-Net-Output, dessen Node schweigt, `Degraded` statt `Ok` — in der Momentaufnahme, im Delta und in der Antwort, damit die drei nicht auseinandergehen. Die Zeile darunter nennt die Node beim Namen, denn *Degraded* allein sagt einem Installateur nicht, zu welcher Kiste er laufen soll.

  Vier Entscheidungen, die dahinterstehen. **Gepollt wird nur dorthin, wo dieses Pult ohnehin sendet** — unicast an die Adressen der Art-Net-Zeilen des Rigs, nie ein Broadcast: ein ArtPoll an eine Adresse, an die 44-mal pro Sekunde 530 Byte gehen, braucht keine neue Erlaubnis und erreicht nichts Neues (S9, S10). Der Preis steht daneben statt versteckt zu sein: eine Node, deren Adresse niemand getippt hat, wird nur gefunden, wenn sie sich **selbst meldet** — was Nodes beim Einschalten tun, und der Socket hört auf Art-Nets eigenem Port zu. **`NodeHealth` ist ein zweites Wort**, keine drei weiteren `OutputHealth`-Varianten: ein Open-DMX-Kabel und ein sACN-Strom haben keine Gegenrede und kämen aus *hat nie geantwortet* nie wieder heraus. Die dritte Form trägt die Zeit als **Alter** in Millisekunden neben sich, nie als Zeitstempel, weil Daemon und Browser keine gemeinsame Uhr haben (S33) — aus *vor 90 000 ms* macht das Panel *20:14*. Und **eine entdeckte Node wohnt nirgends dauerhaft**: sie ist eine Beobachtung über das Netz, also `Query::ArtNetNodes` und nicht `MachineConfig` — sonst änderte sich `machine.json`, weil jemand eine Node ausgeschaltet hat.

  Im Panel stehen jetzt beide Listen und wo sie sich widersprechen: was konfiguriert ist, was im Netz antwortet, welche Universen eine Node ausgibt, auf die dieses Pult nichts sendet, und welche dieses Pult sendet, die die Node nicht hat. Eine entdeckte Node wird mit einem Klick zum Output — als **Formular**, nicht als Kommando, weil ein Panel nicht unaufgefordert das Rig einer laufenden Show ändert. Tests: `a_configured_node_that_never_answers_never_reads_ok` und fünf weitere in `crates/prismd/tests/artnet_nodes.rs` (über das Protokoll, an einem laufenden Daemon), `an_art_net_output_whose_node_never_answers_is_not_reported_ok` in `crates/prismd/src/outputs.rs`, die 21 in `crates/prism-protocols/src/discovery.rs`, `artpoll_wire.rs` über einen echten Loopback-Socket, `artpoll_fuzz.rs` mit einer Viertelmillion zufälliger Bytes und **null** Allokationen, und sieben in `ui/src/settings/settingswindow.test.tsx`.

  **Nachtrag vom selben Tag, aus der Handprobe des Eigentümers:** die erste
  Fassung las eine echte, angeschlossene Node weiterhin als `Degraded`. Der
  Grund war ein **Plattformunterschied**, den kein Test treffen konnte. Ein
  `ArtPollReply` ist laut §6 *mindestens* 239 Byte — das `Filler`-Feld ist
  „transmit as zero, receivers do not test", also polstern Hersteller, und eine
  Antwort mit 240 oder 512 Byte ist eine ganz normale Antwort. Ein `recvfrom` in
  einen Puffer, der kleiner ist als das Datagramm, **schneidet unter Unix ab und
  scheitert unter Windows** (`WSAEMSGSIZE`, und die Daten sind weg). Der Puffer
  war genau 239 Byte groß: unter Linux — also in der CI — lief alles, auf dem
  Release-Ziel wurde jede gepolsterte Antwort verworfen, und die Node antwortete
  brav auf jeden Poll. Gelesen wird jetzt in eine **MTU**, `UdpError::Oversized`
  ist ein eigener Wert, damit ein unlesbares Datagramm verworfen wird statt den
  Empfangsdurchgang zu beenden, und `MockUdpNode` bildet ab sofort Windows nach
  statt Unix — ein Double, das die nachsichtigere Plattform nachbildet, kann
  nicht so scheitern wie das Release-Ziel. Tests:
  `a_node_that_pads_its_reply_is_still_a_node_that_answered`,
  `a_real_node_socket_meets_a_datagram_bigger_than_its_buffer` (über einen echten
  Socket, und **der** Test hätte es gefunden) und
  `a_node_that_pads_its_reply_is_answering_and_not_degraded` über das Protokoll.
  Dazu zeigt das Panel jetzt die **Zähler** — gesendete Polls, Antworten,
  Unlesbares —, weil *es wird gepollt und nichts kommt*, *es wird gar nicht
  gepollt* und *es kommt etwas an und wird verworfen* drei verschiedene Fehler
  mit drei verschiedenen Abhilfen sind und von außen identisch aussahen.

  **Zweiter Nachtrag, und das war die eigentliche Ursache in diesem Saal:** die
  **Windows-Firewall**. Die eingehende Regel für `prismd.exe` galt nur für das
  Profil *Privat*, das Kabel zur Node (`Ethernet 4`) liegt aber im Profil
  *Öffentlich* — also gingen die Polls hinaus und jede Antwort wurde verworfen,
  bevor der Prozess sie sah. Eingegrenzt wurde das mit einer Messreihe: ARP löst
  die Node auf, PowerShell bekommt mit **identischen Bytes auf identischem Port**
  8 von 8 Antworten, `prismd` 0 von 21 — und zwar auch mit **deaktiviertem**
  Output, also ohne ein einziges DMX-Paket auf der Leitung. Damit blieb als
  einziger Unterschied die ausführende Datei, und die Regelliste zeigte die
  Lücke: `prismd.exe` *Privat*, `powershell.exe` *Privat und Öffentlich*.

  Das Pult **konnte das die ganze Zeit sehen** und hat nichts gesagt. Deshalb
  gibt es jetzt `Answer::ArtNetNodes::remedy`: wenn Polls hinausgehen und **kein
  einziges Datagramm** zurückkommt — kein lesbares und kein unlesbares —, sagt der
  Daemon nach drei Polls in eigenen Worten, dass entweder keine Node im Netz ist
  oder die Firewall die Antworten verwirft, und nennt Port 6454. Der Satz
  verschwindet, sobald irgendetwas ankommt. Nach dem Öffnen der Regel:
  `node discovered: "SGM  1" at 2.16.10.66:6454`, eine Antwort pro Poll.

### B19 — Keine doppelten Fixture Profile

- **Wo:** Patch
- **Schwere:** Schönheitsfehler
- **Was passiert:** Fixtures müssen erst der Show hinzugefügt werden, bevor sie gepatcht werden können. Das erzeugt zwei verschiedene Fixture Listen (Library/Show) und ist unnötig
- **Was passieren soll:** Fixtures sollten direkt aus der Library gepatcht werden können (Es reicht auch es nur im Frontend zu machen und das Profil einfach direkt zur Show hinzuzufügen, wenn man es patcht)
- **So sieht man es:** Ein Fixture patchen
- **Ergebnis:** ✅ **behoben** — es gibt nur noch **eine** Liste, und sie steht dort, wo das Profil tatsächlich gewählt wird: im Formular der Zeile, die gerade gepatcht wird. Das Feld *Type* durchsucht die ganze Bibliothek des Pults; was die Show schon trägt, ist markiert statt versteckt und bleibt wählbar — die zweite Lampe eines Rigs ist der häufigste Patch überhaupt. **Das Einbetten passiert beim Auswählen**, nicht beim Apply: die Zeile unter dem Formular ist `Query::PatchPreview`, also die Antwort des Daemons über die *Show*, und ein Profil, das die Show nicht trägt, sagt dort *abgelehnt, 0 Kanäle*. Später einzubetten hieße entweder diese Absage zu zeigen oder den Client entscheiden zu lassen, dass sie nicht zählt — und das ist genau **D3**. Preis: bricht man das Formular nach der Wahl ab, trägt die Show ein ungenutztes Profil; ein Oops weit weg und in einer Datei ohne Gewicht. Tests: `patches straight out of the desk's library, from the row being patched`, `marks a profile the show already carries, and still lets it be chosen` und `opens a row on a show with no profiles at all, because the field is the library` (`ui/src/patch/patchwindow.test.tsx`).

### B23 — Das Fixture-Auswahlfeld im Patch ist unübersichtlich

- **Wo:** Patch, Zeile im Formular
- **Schwere:** ärgerlich
- **Was passiert:** Das Feld ist sehr unübersichtlich
- **Was passieren soll:** Ein Modal, in einer schönen Liste mit abgegrenzten Spalten formatiert
- **So sieht man es:** Eine Patch-Zeile öffnen und das Feld *Type* anfassen
- **Ergebnis:** ✅ **behoben** — B19 hatte aus zwei Listen eine gemacht, aber die Form der einen behalten: ein Textfeld mit einem Dropdown darunter, vier Zeilen `Hersteller · Name · Modus · n ch` in einer Zeile zusammengelaufen, schwebend über der Patch-Tabelle, die es verdeckte. Eine Bibliothek mit zweitausend Profilen ist eine **Tabelle**, und eine Tabelle braucht Platz. Also ist das Feld jetzt eine Anzeige und eine Taste, und die Taste öffnet ein Panel (`chrome/modal.tsx`) mit vier Spalten in der Reihenfolge, in der ein Operator eingrenzt: **Hersteller, Fixture, Modus, Kanäle** — der Modus und die Kanalzahl deshalb als eigene Spalten, weil sich ein 8-Kanal- und ein 15-Kanal-Modus desselben Geräts in einer zusammengelaufenen Zeile gleich lesen und im Rig nicht austauschbar sind. Die Spaltenbreiten sind fest (`table-layout: fixed`), sonst rückt jede getippte Taste die Spalten. Eine Zeile, die die Show schon trägt, sagt es in der letzten Spalte — *picking re-reads it*, was seit B1 die Bedeutung ist. Tests: `patches straight out of the desk's library, from the row being patched` (liest die vier Zellen einzeln) und die vier weiteren in `ui/src/patch/patchwindow.test.tsx`.

## Einstellungen und Pult

*Das Einstellungs-Fenster, die fünf Panels, der Control-Editor, das X-Touch.*

### B3 — Control Binding umgedreht

- **Wo:** Settings Controls Menü
- **Schwere:** ärgerlich
- **Was passiert:** Es werden die Controls des XTouch angezeigt, auf die ich dann eine Aktion binden kann
- **Was passieren soll:** Es werden Aktionen angezeigt, die ich dann durch das Drücken eines gewünschten XTouch Buttons darauf binden kann
- **So sieht man es:** Settings Controls Menü öffnen
- **Ergebnis:** ✅ **behoben** — die Zeilen sind jetzt **Aktionen**, und die Taste kommt vom Drücken. S38 hatte die Tabelle so gezeichnet, wie `prism_surface::Bindings` sie speichert: 73 Zeilen, eine pro Control, mit einem Aktionsmenü darin. Niemand kommt in dieses Panel und will wissen, was F5 tut — man kommt mit *Go auf den gewählten Executor* und braucht eine Taste dafür. Also ist die Liste das Vokabular (`ACTION_GROUPS`, vierundzwanzig Zeilen in vier Gruppen), jede Zeile trägt die Tasten, die auf sie gebunden sind, und **Learn sitzt auf der Zeile**: Learn drücken, gewünschte Taste am XTouch drücken, fertig. Das ist S20s Methodenregel — *drücken statt das Profil fragen* — auf die Frage gerichtet, die der Operator wirklich hat. Eine Zeile ist eine **Art**, keine ganze Aktion: *Fenster öffnen* ist eine Zeile mit einem Fensterfeld daneben, nicht vierzehn Zeilen. Die Custom-Zeile aus Runde 3 ist dieselbe Form: *Kommandozeile schreiben* nimmt den Text, und ein Pult bekommt so viele Tasten mit so vielen verschiedenen Zeilen, wie es braucht — alle unter derselben Zeile aufgeführt. An der Tabelle darunter hat sich nichts geändert: weiterhin ein `MachineChange::SurfaceBinding` pro Control, eine Revision für alle Clients. Tests: die zwanzig in `ui/src/settings/controls.test.tsx` (neu ausgerichtet, nicht gelöscht — darunter *learns a key onto a row, sends one command, and does not move until it is told*, *will not arm a row whose second box is unanswered*, *binds nothing when the desk names a control this panel did not ask for* und *cannot be unbound, wherever it is listed*), `carries every kind except Nothing, exactly once` (`ui/src/settings/actions.test.ts`, die Wächterin gegen eine Vokabel, die aus der Liste fällt) und `ui/e2e/controls.spec.ts` mit echtem Daemon und echter Konsole.


### B4 — Command senden Aktion fehlt

- **Wo:** Settings Controls Menü
- **Schwere:** blocker
- **Was passiert:** Es gibt keine Command senden Aktion
- **Was passieren soll:** Es sollte eine Aktion geben, die das senden eines Commands auf Buttons presses mappt
- **So sieht man es:** Verfügbare Button Aktionen einsehen
- **Ergebnis:** ✅ **behoben** — `SurfaceAction::WriteCommandLine { line }` bindet eine beliebige Kommandozeile auf einen Tastendruck. Sie wird **geschrieben, nicht gesendet**: der Operator sieht auf der Zeile, was gleich läuft, und die Zeile entscheidet weiter wie bei einer getippten (`ARCHITECTURE_SPEC.md` §4.5). Dazu `OpenWindowPicker`, damit der Fensterwähler auch ohne Maus erreichbar ist. **Nachtrag 2026-08-28:** *nicht gesendet* war die halbe Antwort — der Eigentümer hat die andere Hälfte verlangt, und sie ist jetzt ein Kästchen auf der Bindung; siehe **B25**, samt der Begründung, warum die Ausführung vorläufig beim Client mit dem Tastaturfokus liegt. Tests: `crates/prism-surface/tests/bindings.rs` und `ui/src/settings/actions.test.ts`.

### B24 — Die Controls-Seite hat keine Struktur

- **Wo:** Settings, Controls
- **Schwere:** ärgerlich
- **Was passiert:** Die Seite ist sehr unstrukturiert und unübersichtlich, alles hat verschiedene Längen und Breiten
- **Was passieren soll:** Einteilung in Executorbereich, Programmerbereich, Andere Interne Befehle und Custom Befehle; eine Custom-Sektion mit einem `+`-Knopf, in dem der Typ gewählt wird (Send Command / Open Window / Jump to View / Execute Macro); bei Send Command ein Kästchen *nur schreiben* gegen *schreiben und absenden*; die Default-Controls leicht editierbar; Export und Import
- **So sieht man es:** Settings, Controls öffnen
- **Ergebnis:** ✅ **behoben** — und zwei getrennte Ursachen für die eine Beobachtung. **Die Breiten:** die Abschnitte waren vier `<table>`, und eine Tabelle bemisst ihre Spalten selbst — die Spalte *Action* war unter einer Überschrift so breit und unter der nächsten anders, also vier kleine Tabellen statt einer Liste. Jetzt ist es **ein Raster mit einer Spaltenvorlage auf der Zeile**, also vier Spalten in jedem Abschnitt an derselben Stelle; dazu die Kontrollbasis aus B22, die jeder Taste und jedem Feld dasselbe Aussehen gibt. **Die Einteilung** ist die des Eigentümers: `ACTION_GROUPS` sind die drei festen Abschnitte, `CUSTOM_KINDS` der vierte. Der vierte hat eine **andere Form**, und das ist der Punkt: *Fenster öffnen* sind vierzehn Bindungen und *Kommandozeile schreiben* so viele, wie ein Operator sich ausdenken kann — dort ist die **Taste** die Zeile, jede mit ihrer eigenen Antwort, und ein `+` legt eine an. Eine bestehende Custom-Taste wird an Ort und Stelle geändert, ohne neues Learn. **Execute Macro** steht im Wähler und ist abgeschaltet: es gibt keine `SurfaceAction` dafür und kein Makro — ein Eintrag, der nichts bindet, wäre schlechter als einer, der sagt wann. Tests: die sechsundzwanzig in `ui/src/settings/controls.test.tsx`, darunter `adds one with a type, an answer and a key`, `edits a key that is already bound, without a second Learn` und `offers the send box for a line and for nothing else`.

### B25 — Eine gebundene Zeile kann nicht abgeschickt werden

- **Wo:** Settings Controls, Custom Befehle
- **Schwere:** ärgerlich
- **Was passiert:** Eine auf eine Taste gebundene Kommandozeile wird nur geschrieben; Enter muss von Hand kommen
- **Was passieren soll:** Ein Kästchen, das zwischen *nur schreiben* und *schreiben und direkt absenden* umschaltet
- **So sieht man es:** Eine Taste auf `Go Executor 1` binden und drücken
- **Ergebnis:** ✅ **behoben**, und **der Notbehelf ist seit S49 fort**. `SurfaceAction::WriteCommandLine` trägt `submit`, und das Kästchen sitzt auf der Bindung — eine Taste auf `Go Executor 1`, die danach Enter braucht, ist keine Go-Taste, und eine Taste, die `Store Cue ` zum Fertigschreiben hinlegt, ist genau richtig; beides wird gebraucht, also entscheidet die Bindung. **Wer sie ausführt, war die unangenehme Hälfte:** der Parser der Kommandozeile lag in der Oberfläche, also *konnte* der Daemon die Zeile nicht ausführen — er schrieb sie hin, zählte `Session::command_line_run` hoch, und der Client mit dem **Tastaturfokus** zerlegte sie. Auf einem Schirm war das genau richtig; ohne Schirm tat es nichts, und zwei fokussierte Fenster auf zwei Maschinen führten sie zweimal aus. **S49 hat den Parser in den Daemon gezogen** (`prism_core::console`): eine gebundene Zeile ist jetzt `Command::CommandLineInput { run: true }`, der Daemon liest sie und führt sie aus, und das Feld `commandLineRun` gibt es nicht mehr. Tests: `a_key_with_a_line_on_it_fires_the_line` (`crates/prismd/src/core.rs`) — die Taste macht **Licht auf dem Rig**, ohne dass ein Client verbunden ist —, `carries the send box on the binding, both ways` (`ui/src/settings/controls.test.tsx`) und `crates/prism-surface/src/binding.rs`.

### B26 — Controls lassen sich nicht mitnehmen

- **Wo:** Settings, Controls
- **Schwere:** Schönheitsfehler
- **Was passiert:** Eine Tastenbelegung lässt sich weder sichern noch auf ein anderes Pult bringen
- **Was passieren soll:** Export und Import
- **So sieht man es:** Controls einrichten, Daemon neu aufsetzen
- **Ergebnis:** ✅ **behoben** — und was reist, ist eine **Profildatei**: dasselbe Dokument, das `prism_surface::Bindings::parse` liest und in dem `profiles/surface/xtouch.json` geschrieben ist. Ein Export ist damit mehr als eine Sicherung; er lässt sich einem Daemon mit `--surface-profile` geben oder in *Devices* benennen. Der `device`-Schlüssel und die `profileVersion` kommen aus der **Antwort des Daemons** (`Answer::SurfaceBindings`), nicht aus einer zweiten Kopie zweier Konstanten im Client — ein Export gegen eine veraltete Kopie ist eine Datei, die der Daemon danach ablehnt. Beide Richtungen laufen über den **Dateidialog des Browsers** und nicht über einen Pfad, den der Daemon auflöst: eine Tastenbelegung gehört zu der Maschine, an der der Operator sitzt, nicht zu der, die die Show fährt. Ein Import schickt **eine `SurfaceBinding` pro Control** — die Regel des Protokolls, eines nach dem anderen — und zwar für *jedes* Control der Oberfläche, gebunden oder nicht, damit das Ergebnis die Tabelle der Datei ist und nicht die Datei über das, was vorher da war. Tests: `ui/src/settings/controlfile.test.ts` (Rundlauf, und für jede abgelehnte Datei ein Satz statt Schweigen) und `reads a profile back as one binding per control` (`ui/src/settings/controls.test.tsx`).

## Sonstiges

*Alles, was in keinen der Abschnitte darüber passt — Farben, Schrift,
Meldungen, Tastatur, Leerzustände, Verhalten beim Start, Verbindungsabbrüche.*

### B22 — Es gibt noch viele ungestylte Buttons

- **Wo:** überall
- **Schwere:** Schönheitsfehler
- **Was passiert:** Viele Knöpfe sind Browser-Standard: helle, runde Kästchen mit Systemschrift auf einem dunklen, monospaced Pult
- **Was passieren soll:** Keine Default-HTML-Buttons mehr
- **So sieht man es:** Irgendein Store-Bar, irgendein Settings-Panel
- **Ergebnis:** ✅ **behoben** — es gab bis dahin **keine Grundregel** für `<button>`, also kam jeder Knopf ohne eigene Klasse als der des Browsers heraus; etwa sechzig davon. Jetzt gibt es eine, und sie steht in `:where(.desk)`, also mit **Spezifität null**: `.linkish`, `.picker-key`, `.pool-tab` und jede andere benannte Kontrolle gewinnt weiterhin ohne `!important` und ohne längeren Selektor, und ein Knopf hört in dem Moment auf generisch zu sein, in dem er eine Klasse bekommt. Die Alternative — jedem Knopf eine Klasse geben — ist derselbe Stil sechzigmal geschrieben, und der einundsechzigste wird vergessen. Dieselbe Regel für `input`, `select` und `textarea`. Die drei Zustände sind je eine Farbe **und** ein zweites Signal (Rahmen, Füllung, Gewicht), weil ein dunkler Raum keine Farbabgleichübung ist, und der Fokusring ist ein `outline`, damit beim Erscheinen nichts springt.

### B27 — Gruppen-Auswahl toggelt die Fixtures statt der Gruppe

- **Wo:** Group Pool
- **Schwere:** ärgerlich
- **Was passiert:** Wird eine Gruppe selektiert, werden alle Fixtures dieser Gruppe getoggelt — waren sie vorher an, gehen sie aus
- **Was passieren soll:** Beim Selektieren sollen nur Fixtures hinzugefügt werden. Wird die Gruppe deselektiert, werden nur die Fixtures deselektiert, die nicht durch andere selektierte Gruppen oder manuell selektiert sind
- **So sieht man es:** Zwei überlappende Gruppen nacheinander drücken
- **Ergebnis:** ✅ **behoben** — eine Gruppe ist jetzt ein **Schalter** und kein Fixture-Toggle. Drücken schaltet sie ein; nochmal drücken schaltet sie aus und nimmt nur die Fixtures zurück, die **keine andere eingeschaltete Gruppe und keine direkte Auswahl** mehr hält. Diese Frage lässt sich aus der Auswahlliste allein nicht beantworten — ein Fixture darin sagt nichts darüber, wer es hineingelegt hat — also führt der Programmer die Herkunft mit: `selectedGroups` und `manualSelection`, beide auf der Leitung, beide `#[serde(default)]`. Sie sind **die des Daemons**, nicht des Clients (**D3**): welche Fixtures eine Gruppe hält, ist Show-Zustand, ein Client, der die Subtraktion selbst rechnete, schickte eine Auswahl, die die Änderung eines zweiten Clients schon falsch gemacht hat. Die Zeile ist unverändert `+ Group 3`; was sie bedeutet, entscheidet der Daemon (`Programmer::select_group`). Tests: `a_second_group_does_not_punch_a_hole_in_the_first`, `switching_a_group_off_keeps_what_another_group_still_holds`, `switching_a_group_off_keeps_what_was_picked_by_hand`, `switching_a_group_off_reads_the_group_as_it_is_now` und `clearing_the_selection_lifts_every_group_switch` (`crates/prism-core/src/programmer.rs`) sowie `lights the groups the programmer says are on, and nothing else` (`ui/src/show/grouppool.test.tsx`).

### B28 — Der Fensterwähler ist zu schmal

- **Wo:** Fensterwähler (Insert)
- **Schwere:** Schönheitsfehler
- **Was passiert:** Eine lange Liste statt mehrerer Knöpfe nebeneinander
- **Was passieren soll:** Breiter, mit mehreren Knöpfen nebeneinander
- **So sieht man es:** Insert drücken
- **Ergebnis:** ✅ **behoben** — es war seit B9 ein Raster, aber in einem Panel, das schmal genug war, dass `auto-fill` immer nur zwei Spalten gab; es las sich als Liste mit einer Lücke in der Mitte. Das Panel ist jetzt das **breite** Modal, und jede Taste trägt eine eigene Zeile, die sagt, was das Fenster ist — die andere Hälfte davon, dass ein Wähler ein Wähler ist: vierzehn Namen, die man schon kennen muss, sind ein Menü, das man durch zufälliges Öffnen lernt. Die Sätze stehen in `windowNote` und nicht in `windowTitle`, weil ein Titel das ist, was ein Fensterrahmen zeichnet. Der `switch` ist über die generierte Union **erschöpfend**, also fällt ein später hinzugefügter Fenstertyp hier beim Kompilieren auf statt mit leerer Zeile im Wähler zu stehen.

### B29 — Cue Viewer: eine Zeile pro Wert statt pro Cue

- **Wo:** Cue Viewer
- **Schwere:** ärgerlich
- **Was passiert:** Eine Zeile pro Wert — Cue, Fixture, Attribut, Wert, Link
- **Was passieren soll:** Nur eine Zeile pro Cue, und für jedes Attribut eine Spalte
- **So sieht man es:** Cue Viewer bei einer Liste mit mehreren Cues öffnen
- **Ergebnis:** ✅ **behoben** — eine Liste von vierzig Cues über zwanzig Lampen sind mehrere tausend Zeilen davon, und die Frage, mit der man dieses Fenster öffnet — *was ändert Cue 7, und was lässt er stehen?* — kostete hundert Zeilen Scrollen. Jetzt: Cue = Zeile, Attribut = Spalte. Eine Zelle sagt eines von drei Dingen. **Unberührt** — ein Strich: der Cue erwähnt das Attribut nicht, also bleibt stehen, was ein früherer Cue gesetzt hat. Das ist genau die Lesart, die **S48** braucht, und dieses Fenster ist das, in dem sie gelesen wird. **Ein Wert** — die Prozentzahl. **Mehrere** — die Spanne `10–80 %`; kein Mittelwert, denn das ist eine Zahl, auf der kein Fixture steht, und nicht der erste, denn das wäre eine Behauptung über die anderen neunzehn; die Einzelwerte stehen im Titel. Die Spaltenreihenfolge ist `AttributeType::ALL`, also die generierte, denn das Pult hat **eine** Reihenfolge für Attribute. Tests: die fünf in `describe("the cue grid")` (`ui/src/show/sequencesheet.test.tsx`).

### B30 — Sequence Sheet, Group Sheet und Preset Sheet sollen Grids werden

- **Wo:** Sequence Sheet, Group Pool, Preset Pool
- **Schwere:** ärgerlich
- **Was passiert:** Das Sequence Sheet zeigt Details, Eingabefelder und die Cue-Tabelle; die kleineren Aktionen hängen als Knöpfe an den Kacheln
- **Was passieren soll:** Grids. Das Sequence Sheet wird reine Auswahlfläche, alle Cue-Editierungen ziehen in den Cue Viewer, und Namens- oder Farbänderungen kommen per Rechtsklick. Beim Preset-Fenster kommt ein **Multi**-Preset dazu, das kategorieübergreifend funktioniert
- **So sieht man es:** Die drei Fenster öffnen
- **Ergebnis:** ✅ **behoben** — und die Linie zwischen zwei Fenstern hat sich verschoben, was die eigentliche Änderung ist: alles unterhalb der Sequenz-Leiste handelte von **einer** Liste, die Leiste selbst von *welcher*. Das sind zwei Fenster. Das **Sequence Sheet** ist der Pool: Kacheln, mehr nicht. Der **Cue Viewer** hat die Cue-Tabelle, den Executor-Transport und die Store-Bar bekommen. Die drei Pools zeichnen jetzt dieselbe Kachel (`.pool-box`), und Umbenennen, Färben, Kopieren, Verschieben und Löschen kommen aus einem gemeinsamen Menü (`chrome/menu.tsx`) — dasselbe, das die View-Leiste seit S35 benutzt, hierfür herausgezogen statt ein viertes Mal geschrieben. **Farbe für Presets ist neu und war eine Lücke:** `Preset::color` steht seit S11 auf der Leitung und **nichts konnte sie setzen** — `Command::Color` nahm nur Sequenzen —, also zeigten die Scribble-Strips eine Farbe, die kein Operator wählen konnte. **Multi** ist eine `PresetPool` und keine achte `FeatureGroup`: es gibt keine Encoder-Bank, die *Multi* sein könnte, und kein Attribut, das dazu gehörte. Der Daemon brauchte dafür keinen neuen Filter — *kein Pool genannt* ist der Filter, den ein **Cue** seit S28 nimmt. Tests: `preset_pool_is_the_banks_and_multi` (`crates/prism-domain/src/preset.rs`), `a_multi_preset_takes_every_value_and_a_colour_one_takes_the_colour` (`crates/prism-core/src/programmer.rs`), `manages a cue list from a right-click, with the lines every pool shares` und `deletes and renames from the menu, with the lines every pool shares`.

### B32 — Store-Sektionen im Group Sheet und Preset Pool

- **Wo:** Group Pool, Preset Pool
- **Schwere:** Schönheitsfehler
- **Was passiert:** Beide Fenster tragen unten eine Store-Leiste
- **Was passieren soll:** Entfernen; gestort wird nur über die Command Line
- **So sieht man es:** Beide Fenster öffnen
- **Ergebnis:** ✅ **behoben** — und es kostet nichts, weil die Leiste ohnehin schon eine Zeile schrieb: sie baute aus einer Nummer, einem Namen und einem Modus `Store Group 4 "Front wash"` und schickte es ab. Sie war also ein zweiter Weg, einen Satz zu tippen, und nahm dafür ein Drittel des Fensters. Sie konnte sogar **weniger** als die Zeile: ihre Nummernbox bot nur die nächste freie an, während `Store Group` plus ein Klick auf eine Kachel jetzt `Store Group 3` ergibt (siehe B33). Was verloren geht, ist die vorgeschlagene Nummer — das steht im Zähler über dem Grid, und die Konsole fragt, bevor sie etwas überschreibt. Beim Preset-Pool trug die Leiste zusätzlich die Antwort von `Query::StorePreview` auf dem Knopf; die ist **nicht** weg, sondern ist die **Rückfrage** der Konsole (S39/S40) — dieselbe Frage, dieselbe Query, nur ohne eigenes Panel. Neun Tests, die die Boxen beschrieben, sind mit der Leiste gegangen; was sie behaupteten, steht als Absatz in beiden Testdateien, damit niemand danach sucht. Neue Tests: `has no store bar at all` in `ui/src/show/grouppool.test.tsx` und `ui/src/show/presetpool.test.tsx`.

### B33 — Datenquellen sollen an die Command Line anhängen

- **Wo:** Sequence Sheet, Fixture List, Cue List, Group Sheet, Preset Pool, Executors
- **Schwere:** ärgerlich
- **Was passiert:** Ein Klick führt immer *select* aus, auch wenn in der Command Line schon ein Befehl steht
- **Was passieren soll:** Solange ein passender Befehl in der Zeile steht, soll die Datenquelle **angehängt** werden — z.B. steht `Store` in der Zeile und man klickt auf Sequence 2, dann wird `Sequence 2` angehängt. Nimmt der Befehl keine weiteren Argumente, soll er sofort abgeschickt werden
- **So sieht man es:** `Store` tippen und auf eine Sequence klicken
- **Ergebnis:** ✅ **behoben** — `consoleshell.ts::pickOnto`, eine reine Funktion, und `ARCHITECTURE_SPEC.md` §4.5 damit zu Ende gedacht statt erweitert: die Regel war schon *jede Taste schreibt eine Zeile*, und was fehlte, ist, dass die Pools auch Tasten sind. Wer ein Verb getippt hat, **verlangt ein Argument**, und der schnellste Weg, an einem Pult eines zu geben, ist draufzuzeigen.

  **Was *passend* heißt, entscheidet das Verb.** `VERB_WORDS` sind die Kopfwörter des Parser-`switch`: eine Zeile, die mit einem davon beginnt, tut etwas mit einem *benannten Objekt*; eine, die es nicht tut, ist eine Fixture-Auswahl. `1 thru` plus Klick auf eine Gruppe ist ein Bereich im Bau und keine Gruppe, die benannt wird — also bleibt die Kachel die Kachel. Welche *Nomen* ein Verb nimmt, bleibt Sache der Grammatik und wird nicht hier nachgebaut: `Store Fixture 5` bleibt stehen, mit der Beschwerde des Parsers darunter, statt das getippte `Store` heimlich wegzuwerfen und ein Fixture auszuwählen — genau das Verhalten, das dieser Eintrag beseitigen soll.

  **Sofort abgeschickt** wird, was zu einer ganzen Zeile geworden ist — mit einer Ausnahme, und die ist die wichtige: `Label` und `Color` sind auch ohne letztes Wort gültige Befehle, die etwas *wegnehmen* (`CLEARING_VERBS`), also werden sie angehängt und stehen gelassen. Ein Klick, der einen Namen löscht, wäre die schlechteste Art von Abkürzung. Und *sofort* heißt nicht *ohne Rückfrage*: `Store Group 3` auf eine Gruppe, die es gibt, stellt die Modusfrage der Konsole, genau wie die getippte Zeile.

  Tests: die neun in `ui/src/desk/consoleshell.test.ts` — darunter `partitions every word the console knows`, die Wächterin gegen ein Verb, das in die Grammatik kommt und nicht in die Tabelle — sowie `finishes a waiting line and sends it at once`, `asks before it overwrites, exactly as the typed line does`, `does the box's own thing when the line cannot take a group` und `leaves a clearing verb standing rather than sending it` in `ui/src/show/grouppool.test.tsx`.

### B31 — Dateidialoge des Betriebssystems

- **Wo:** Settings, Show files
- **Schwere:** Schönheitsfehler
- **Was passiert:** Pfade werden getippt
- **Was passieren soll:** Öffnen und Speichern jeder Art (Showfiles/JSON/Control Map) im Dateimanager des ausführenden Betriebssystems. *Sollte das erst mit der Desktop Umgebung gehen, muss das Feature in diese Session verschoben werden*
- **So sieht man es:** Settings, Show files, *Save as*
- **Ergebnis:** ✅ **behoben in S29** — verschoben worden war der Eintrag auf die vom Eigentümer selbst genannte Bedingung, und die Bedingung ist jetzt erfüllt: es gibt eine Desktop-Umgebung. `crates/prism-app/src/dialogs.rs` hält die Tabelle — welcher Ort welchen Dialog öffnet, mit welchen Filtern, als Öffnen, Speichern oder Verzeichnis — und ein Tauri-Kommando gibt den gewählten Pfad als Zeichenkette zurück. Sieben Orte: die fünf Dateibefehle des Protokolls (*Open*, *Save as*, *New*, *Export JSON*, *Import JSON*), die Fixture-Bibliothek und die Bindungsdatei in *Devices*.

  **Am Protokoll ändert sich nichts**, und das ist die Messung, die sagt, dass die Form stimmte: gesendet wird derselbe `OpenShow` mit demselben `path`, ob jemand ihn getippt oder ausgewählt hat. Der Dialog füllt das **Feld** und schickt nichts ab — der Apply-Knopf bleibt, also wird ein versehentlich gewählter Pfad genauso korrigiert wie ein versehentlich getippter, und der Hinweis unter dem Feld (*ein Open legt nie eine Datei an*) wird weiterhin gelesen, bevor etwas passiert. Er öffnet dort, wo die offene Show liegt, statt dort, wo zuletzt ein anderes Programm benutzt wurde.

  **Das Textfeld bleibt**, und der *Browse…*-Knopf wird nur gezeichnet, wo etwas dahinter ist (`ui/src/shell/bridge.ts::inShell`). Ein Knopf, der im Browser nichts tut, wäre genau der Fehler, gegen den S37 die *held by a flag*-Zeilen gebaut hat — und das Web Remote (S31) ist ein Browser und bleibt einer.

  **Die Control Map ist die Ausnahme und war es schon** (B26): sie geht durch den Dateidialog des Browsers, weil eine Tastenbelegung zu der Maschine gehört, an der der Operator sitzt. `dialogs.rs` hat dafür bewusst keinen Eintrag.

  Tests: `every_place_a_path_is_asked_for_has_a_dialogue`, `a_suggested_name_and_its_filter_agree`, `the_fixture_library_is_the_only_directory`, `every_file_dialogue_can_be_made_to_show_everything` und `the_five_show_kinds_are_the_protocols_five_file_commands` (`crates/prism-app/src/dialogs.rs`), die zwölf in `ui/src/shell/bridge.test.ts`, sowie *offers no file dialogue in a browser, and the box still works*, *puts what the operating system's dialogue returned into the box the daemon is sent* und *leaves the box alone when the operator cancels the dialogue* in `ui/src/settings/settingswindow.test.tsx`.

### B34 — Ein Rig aus RGBW-PARs geht beim Start des Daemons an

- **Wo:** Ausgabe, jede Show mit farbmischenden Fixtures ohne Dimmerkanal
- **Schwere:** **schwerwiegend** — das Pult macht Licht, das niemand programmiert hat
- **Was passiert:** B1 gab Farbkanälen den Ruhewert *voll*, weil eine Farbe am Pult offen anfängt und der Dimmer entscheidet, ob man sie sieht. Ein RGBW-PAR hat keinen Dimmer: bei ihm sind *Farbe offen* und *Lampe voll* dieselben acht Bit. Drei PARs auf `show-rig.prism` standen im Ruhezustand auf zwölf Kanälen à 255 — gemessen an der Telemetrie-Leinwand, 1672 Pixel Vollausschlag am Universe-Meter, unverändert durch jeden Programmer-Befehl.
- **Was passieren soll:** Fixtures ohne Dimmer bekommen einen softwaregesteuerten Dimmer, pro Fixture im Patch abschaltbar (Entscheidung des Eigentümers)
- **So sieht man es:** Daemon mit `show-rig.prism` starten, DMX Sheet öffnen, nichts programmieren
- **Ergebnis:** ✅ **behoben** — das Pult liefert den fehlenden Kanal. `Fixture.softwareDimmer` (Vorgabe *an*, `serde(default)`, damit eine ältere Show dunkel hochkommt) und `FixtureType::has_dimmer()` sind die zwei Hälften einer Frage, die `Fixture::has_software_dimmer` zusammen stellt — beide zusammen, weil die eine Hälfte allein jedes Mal die falsche Antwort gibt.

  **Ein Merge-Slot ohne Kanal.** `MergePlan::build` legt für so ein Fixture einen `Dimmer`-Slot an, der bei **null** ruht: Playbacks schreiben ihn, die Master skalieren ihn (er ist `FeatureGroup::Dimmer`), der Programmer übernimmt ihn — nur eine DMX-Adresse hat er nicht. `ChannelPlan` löst stattdessen zur Patch-Zeit auf, welcher Slot welche Kanäle skaliert, und der Tick trägt einen Index pro Ziel: `value * dimmer / 65535`, vor dem Invert, weil der Dimmer davon handelt, wie viel Licht das Fixture macht, und ein Invert davon, wie es verdrahtet ist. Skaliert werden die **Farbkanäle** — bei einem Fixture ohne Intensität ist die Farbe die Intensität; einen Gobo zu dimmen hieße, einen Spiegel zu dimmen.

  **Gefragt wird nach dem Attribut, nicht nach der Bank.** *Intensität* heißt sonst überall `FeatureGroup::Dimmer`; hier nicht, und der Unterschied ist ein Profil, das seinen Dimmer auf der Farbbank ablegt. So eines **hat** einen `AttributeType::Dimmer`, und ein zweiter wäre `MergeError::DuplicateAttribute` — ein Rig, das sich nicht patchen lässt. Ein Test in `ui/src/desk/programmer.test.ts` hatte genau so ein Profil und hat den Fehler gefunden, bevor er ausgeliefert wurde.

  **Der Preis, und er ist der richtige:** ein RGBW-PAR macht von Farbe allein kein Licht mehr. `1 red at 100` färbt, `1 at 100` macht hell — genau wie bei einem Fixture mit echtem Dimmer. Drei e2e-Tests führten den Dimmer nicht hoch und sind mitgezogen worden.

  Tests: `the_desk_supplies_an_intensity_only_where_the_profile_has_none` und `a_fixture_read_without_the_field_gets_the_supplied_intensity` (`prism-domain`), `a_supplied_intensity_is_a_slot_that_rests_at_nought` und `a_fixture_that_was_not_given_one_has_the_slots_it_always_had` (`prism-engine::plan`), `the_supplied_intensity_scales_the_colour_and_nothing_else`, `a_fixture_with_the_switch_off_is_written_straight_through` und `scaling_is_exact_at_both_ends` (`prism-engine::encode`), `a_colour_only_fixture_has_an_intensity_the_programmer_can_reach` und `a_fixture_with_the_supplied_intensity_switched_off_has_none` (`prism-core`), `supplies an intensity for a fixture whose profile has none` und `supplies nothing where the operator switched it off, or where there is one already` (`ui/src/desk/programmer.test.ts`), `offers the desk's dimmer only to a fixture whose profile has none` (`ui/src/patch/patchwindow.test.tsx`). Spezifikation: `docs/DMX_MERGE.md` §5.1.
