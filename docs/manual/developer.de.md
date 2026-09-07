# PrismDMX — Handbuch für Entwickler

**Für:** wer den Quelltext ändern will.
**Gilt für:** `0.9.2`.
**Zur Sprache:** Dieses Handbuch gibt es auf Deutsch und auf Englisch, aber
**der Quelltext ist englisch** — jeder Bezeichner, jede Commit-Nachricht, jede
Spezifikation. Wo dieses Handbuch einen Typ, eine Funktion oder einen Test
nennt, steht der englische Name unübersetzt da, weil das der Name ist, nach dem
Sie greifen werden. [`README.md`](README.md) hält die Entscheidung fest.

Das hier ist nicht die Spezifikation.
[`ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) sagt, was das System *ist*;
das hier sagt, wie man daran arbeitet — die Form, die es tatsächlich angenommen
hat, die Regeln, die aus Fehlern entstanden sind, und vier durchgearbeitete
Rezepte für die vier Dinge, die am häufigsten hinzukommen.

---

## Inhalt

1. [Einen Build bekommen](#1-einen-build-bekommen)
2. [Die Form, die es angenommen hat](#2-die-form-die-es-angenommen-hat)
3. [Die Regeln, die aus Fehlern entstanden sind](#3-die-regeln-die-aus-fehlern-entstanden-sind)
4. [Ein Command hinzufügen](#4-ein-command-hinzufügen)
5. [Einen Fenstertyp hinzufügen](#5-einen-fenstertyp-hinzufügen)
6. [Einen Fixture-Typ hinzufügen — oder dem Bibliotheks-Reader etwas beibringen](#6-einen-fixture-typ-hinzufügen--oder-dem-bibliotheks-reader-etwas-beibringen)
7. [Eine Ausgangsart hinzufügen](#7-eine-ausgangsart-hinzufügen)
8. [Die Gates, und wofür jedes da ist](#8-die-gates-und-wofür-jedes-da-ist)
9. [Testregeln, über die nicht verhandelt wird](#9-testregeln-über-die-nicht-verhandelt-wird)
10. [Wie Arbeit festgehalten wird](#10-wie-arbeit-festgehalten-wird)

---

## 1. Einen Build bekommen

Sie brauchen [Rust](https://rustup.rs) stable (1.89 oder neuer),
[Node](https://nodejs.org) 24 und — unter Windows — die MSVC-Build-Tools. Unter
Linux braucht der Bau der Shell zusätzlich ein WebView-Toolkit
(`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`); nichts sonst im Workspace braucht es.

```bash
git clone https://github.com/flakesystems/PrismDMX.git
cd PrismDMX
tools/fetch-fixtures/fetch-fixtures.sh   # oder .ps1 unter Windows
cd ui && npm ci && cd ..
cargo build --workspace
```

Zwei dieser Schritte sind je einen Satz wert.

**Die Fixture-Bibliothek wird geholt, nicht mitgeliefert.** Sie hat ein Upstream
mit eigenem Veröffentlichungstakt, und eine Kopie in diesem Baum wäre die
veraltete. Das Skript nagelt eine Revision fest, damit zwei Rechner, die an
verschiedenen Tagen installieren, dieselben Profile bekommen. Die Corpus-Tests
**überspringen sich selbst**, wenn sie fehlt — deshalb holt sie jeder CI-Job,
der sie braucht, vorher.

**`ui/src/bindings/` wird erzeugt, nicht mitgeliefert.**
`cargo test -p prism-domain` schreibt es aus den Rust-Typen, sodass eine
veraltete Bindung keinen grünen Build überleben kann; und nichts in `ui/`
typecheckt, bevor das einmal gelaufen ist.

Die beiden Hälften beim Entwickeln getrennt starten:

```bash
cargo run -p prismd -- --mock-output
npm --prefix ui run dev
```

---

## 2. Die Form, die es angenommen hat

### Zwei Prozesse, und die Grenze ist der Punkt

`prismd` besitzt die Show, die Session, die Engine und jeden Ausgang. Die Shell
und die Oberfläche besitzen ein Fenster. **Stirbt die Oberfläche, läuft das
Licht weiter** — der Operator macht am X-Touch weiter, und wenn die Oberfläche
zurückkommt, findet sie den Zustand vor, den die Konsole hergestellt hat. Das
ist Entscheidung **D2**, und alles andere folgt daraus: die Engine hat
überhaupt keine Oberfläche *in sich*.

### Neun Crates, in Abhängigkeitsreihenfolge

```
prism-domain ── die Wörter
   ├── prism-engine ──── Tick, Merge, Kodierung                (gar kein I/O)
   ├── prism-core ────── Show, Programmer, Kommandozeile
   ├── prism-protocols ─ Art-Net, sACN, Open DMX USB
   ├── prism-surface ─── das X-Touch, in drei Schichten        (kein Port)
   ├── prism-midi ────── der Port                              (kein Protokoll)
   └── prism-ipc ─────── Framing und Transporte
            └── prismd ──── der Daemon: alles davon, verdrahtet
                    └── prism-app ── die Shell und der Installer
```

Jedes Crate hat eine `README.md`, die sagt, wofür es da ist, was es **nicht**
enthalten darf und wie man es testet. Lesen Sie die vor dem Quelltext.

### Wo Zustand lebt, und die drei Applier

Es gibt genau drei Arten von Zustand, und jedes Command gehört zu genau einer —
`crates/prism-core/tests/command_application.rs` sichert das, indem es zählt.

| Zustand | Lebt in | Gespeichert in | Beispiel |
|---|---|---|---|
| **Die Show** | `prism_core::Show` | der `.prism`-Datei | Fixtures, Cues, Gruppen, Presets, Executors |
| **Die Session** | `prism_core::SessionState` | der `.prism`-Datei, neben der Show | aktiver View, offene Fenster, Seite, Auswahl, Kommandozeile |
| **Die Maschine** | `prism_core::MachineConfig` | `machine.json` | Ausgangs-Patch, Pult-Identität, Surface-Port, Netzwerkfreigabe |

Eine vierte Art gibt es, und sie hat **keinen** Applier: `Shutdown` wirkt auf
den *Prozess*. Sie wird in jenem Test trotzdem mitgezählt, denn eine Art, deren
Zugehörigkeit implizit wäre, wäre ein Loch von genau der Größe der Prüfung.

Zwei Folgerungen, über die Leute stolpern:

- **Die Session steckt in der Show-Datei.** Ein Layout reist also mit einer Show
  mit. Was dort ausdrücklich *nicht* steht — Monitorzuordnung, Scrollposition,
  Kamera, welcher Einstellungs-Tab offen ist — ist client-lokal, nach
  `ARCHITECTURE_SPEC.md` §4.2.
- **Der Rig steckt *nicht* in der Show-Datei.** Eine Show, die auf einem Stick
  in ein anderes Haus getragen wird, darf die Verkabelung des ersten Hauses
  nicht mitbringen. Das ist die Entscheidung aus §7.0, und es ist dasselbe
  Argument wie beim sACN-CID.

### D11: die Konsole bedient die Oberfläche

Der aktive View, die offenen Fenster, die Executor-Seite und die Auswahl sind
**Session-Zustand im Daemon**. Genau das erlaubt einem X-Touch, einen View
umzuschalten, während kein Client läuft — und macht einen Client-Neustart zu
einem gewöhnlichen Reconnect statt zu einer Wiederherstellung. Dafür gibt es ein
Gate in der CI: eine Mock-Surface drückt zwei Tasten, **ohne dass ein Client
verbunden ist**, und ein Client, der sich danach verbindet, findet View und
Fenster in seinem Snapshot.

### Die Kommandozeile ist die Bedienung

`ARCHITECTURE_SPEC.md` §4.5. Jede Taste, jede Kachel und jeder Knopf der
Oberfläche **schreibt eine Zeile**, statt zu handeln — dadurch sind eine Geste
und ein getippter Satz dasselbe, und jede Zeile lässt sich auf eine X-Touch-Taste
legen. Der Parser ist `prism_core::console`, seit S49 im Daemon, weil eine Taste
auf einer Steuerfläche keine Zeile ausführen kann, deren einziger Parser in einem
Browser sitzt.

Zwei Regeln tragen das, und beide sind tragend:

- **Der Parser liest die Show nicht.** Was eine Zeile *bedeutet*, hängt nur von
  der Zeile ab. Was es *gibt*, gehört dem Daemon, und eine Ablehnung ist eine
  Nachricht und keine Exception. (Das einzige, was *doch* gelesen wird, ist, ob
  ein Ziel belegt ist — und auch das nur, um zu entscheiden, ob gefragt wird.)
- **Der Parser scheitert nie.** Auf jede Zeichenkette, die es gibt, antwortet er
  mit Commands oder mit einem Satz, den man jemandem zeigen kann. Eine Konsole,
  die bei einem Tippfehler mitten in der Show werfen könnte, ist eine Konsole,
  die aufhört zu antworten.

### Der Tick

44 Hz, eigener Thread, eigene Priorität. Er nimmt kein Lock, wartet auf nichts
und **macht keinen Allokator-Aufruf** — gemessen auf zehn Pfaden, mit einem
elften Test, der absichtlich alloziert, damit die Sonde nicht stillschweigend
kaputtgehen kann. Alles, was ein `Vec` braucht, passiert auf dem Core-Thread und
erreicht den Tick hinter einem Atomic.

Frames verlassen ihn durch einen Triple-Buffer, einen Subscriber je
Ausgangstreiber. Einen Ausgang im laufenden Betrieb hinzuzufügen oder zu
entfernen kostet die Ausgänge, die sich nicht geändert haben, **keinen Tick und
kein Frame**.

---

## 3. Die Regeln, die aus Fehlern entstanden sind

Jede davon steht in `PROGRESS.md` §6, mit der Session, die sie gelernt hat, und
dem Test, der es gefangen hat. Sie stehen hier, weil sie sich verallgemeinern —
und weil jede von ihnen mindestens einmal in einer zweiten Gestalt neu gelernt
werden musste.

### Eine Regel, die in zwei Schichten gefragt wird, ist eine Funktion und kein `match`

*Nichts bewegt je einen Crossfade-Fader* ist eine Aussage über die Oberfläche
**und** über den Browser. Zweimal geschrieben, laufen die beiden beim ersten
neuen Fader-Modus auseinander: ein `match` bekommt einen neuen Arm, das andere
fällt in den falschen Default. `ExecutorFaderFunction::desk_may_move_it` ist die
Form — ein Prädikat in `prism-domain`, von beiden Seiten gefragt, und ein Test,
der `ALL` abläuft statt der Varianten, an die sich jemand erinnert hat.

Dieselbe Form findet sich als `AttributeType::is_additive_emitter` und als
`crossfade_mode`. Wenn Sie das zweite `match` über ein Enum schreiben, das
anderswo schon eines hat, ist das das Signal.

### Eine Regel über eine Kategorie ändert ihre Bedeutung, wenn die Kategorie wächst

*Eine Farbe ruht offen* war implementiert als *die Farbbank ruht auf voll*, und
das waren dieselbe Aussage, solange jede Farbe im Modell ein additiver Emitter
war. CMY dazuzunehmen — Filter, bei denen voll undurchsichtig heißt — ließ jeden
CMY-Rig zu Hause **schwarz** hochkommen, und nichts wurde rot, bis ein
Corpus-Test nach einem Fixture fragte, auf das niemand sah.

Fragen Sie das **Attribut**, nicht die Bank; fragen Sie die Sache selbst, nicht
die Gruppe, in der sie heute zufällig steckt.

### Ein neuer Konstruktor neben einem alten ist ein neuer Weg, jede alte Regel zu brechen

Ein aufgelöster Switching-Alias, gebaut mit dem Raw-Kanal-Konstruktor, ruhte auf
null; ein Rot muss offen ruhen. Die Regeln, die der alte Konstruktor durchsetzte,
wurden zu einer Liste, an die niemand den neuen hielt. Taucht ein zweiter Weg
auf, einen Typ zu bauen, dann suchen Sie, was der erste stillschweigend
garantiert hat.

### Eine Invariante über das, was eine Sache *erzeugt*, schlägt jede Zahl von Zählern über das, was sie wegwirft

Vier Zähler standen auf null, während 707 DMX-Slots der Bibliothek keinen Knopf
hatten — denn ein Slot, den der Reader nicht verstand, wurde schlicht nie
aufgeschrieben; die Abwesenheit hatte keinen Namen, also zählte sie niemand. Die
Aussage, die es fand, stellt die Frage des Operators: *hat jeder Slot jedes
Profils genau ein `AttributeDef`?*

Zähler sind Diagnose. Sie können nicht die Garantie sein.

### Ein Boden, keine Liste von Reparaturen

Genau die vier Ursachen eines unerreichbaren Slots zu beheben hätte den Corpus
grün gemacht und die fünfte — ein Capability-Typ, den das Format nächstes Jahr
bekommt — durch dasselbe Loch fallen lassen, lautlos; genau so waren die vier
hineingekommen, eine Session nach der anderen. `AttributeType::Raw` ist ein
**Default**: was kein anderes Attribut erreicht, erreicht ihn.

### Ein handgeschriebener Diff ist eine Liste, an die niemand die Struktur hält

`SessionState::commit` vergleicht Feld für Feld. Ein Feld, das der Struktur, dem
Command und dem Applier hinzugefügt wird — aber nicht dem Diff — bewegt die
Session und sendet **kein Delta**; ein zweiter Bildschirm sitzt dann auf altem
Zustand, und das Pult, an dem getippt wurde, sieht richtig aus. Das einzige, was
das fängt, ist `session_deltas_reproduce_the_session_they_came_from`: eine
Eigenschaft über beliebige Commands, die jedes gesendete Delta in einen Spiegel
zurückspielt und vergleicht.

Fügen Sie einem persistierten oder gespiegelten Typ ein Feld hinzu, ist die
Checkliste: die Struktur, das Command, der Applier, **der Diff**, die
Klassifikationsprädikate, das TypeScript — und **der handgeschriebene Decoder in
`ui/src/ipc/protocol.ts`**, für den die generierte Definition nicht einspringt.

### Ein optionales Feld ist ein Feld, das ein handgeschriebener Decoder vergessen darf

`occurrence` (S52) erreichte die Struktur, das Command, den Applier, den
Serialisierer und das generierte TypeScript — und **nicht**
`readProgrammerEntry` in `ui/src/ipc/protocol.ts`, das von Hand geschrieben ist,
weil es ungeprüfte Eingaben validiert. Der Daemon schickte `White` Vorkommen 1,
die Oberfläche legte es unter 0 ab, und ein Kopf mit einem warmen **und** einem
kalten Weiß führte das kalte auf der Lampe aus, während die Zahl unter dem
gedrehten Encoder stehen blieb.

Unsichtbar gemacht hat es die Regel darunter, die funktionierte: das Feld ist
`#[serde(default, skip_serializing_if = …)]`, damit eine `.prism`-Datei von vor
S52 noch öffnet, also ist die Definition `occurrence?: number`, **also ist ein
Objekt ohne dieses Feld ein gültiges `ProgrammerEntry` und der Compiler hatte
nichts zu sagen.** Die Optionalität, die es für eine alte Datei gibt, hat das
Vergessen erlaubt.

Zweierlei folgt daraus. Ein Feld an einem gespiegelten Typ gibt der Checkliste
oben einen siebten Punkt: **den handgeschriebenen Decoder**. Und ein Decoder
soll den Vorgabewert *normalisieren*, statt das Loch weiterzureichen —
`readOccurrence` antwortet `0`, sodass kein Leser sich `?? 0` merken muss, und
der zweite Leser ist der, der es vergisst.

### Ein fehlendes Feld ist die bessere Migration als eine Migration

`occurrence` musste sieben Typen erreichen — der Schlüssel, unter dem jeder Wert
einer Show abgelegt ist. Es kam als `#[serde(default, skip_serializing_if = …)]`
und **nullbasiert** hinein, sodass *fehlt* und *das erste* dieselbe Aussage sind:
eine Show ohne Wiederholungen serialisiert Byte für Byte wie zuvor, eine ältere
Datei öffnet mit jedem Wert dort, wo er war, und es gibt überhaupt keine
Migrationszeile.

### Der eigene Zähler eines Konverters kann nicht sagen, dass seine Ausgabe angenommen wird

Der Bibliotheks-Reader nummerierte Wiederholungen richtig und sein
Duplikat-Zähler stand auf null — und ein Profil von 2 871 wurde dann von der
*Show* abgelehnt, deren Validator noch die alte Frage stellte. Schicken Sie den
ganzen Corpus durch die Tür, die der echte Aufrufer benutzt.

### Eine Geste, die keinen Wert bewegt, darf keinen Zustand wegnehmen

Das Umsortieren der Clear-Stufen machte den ersten Druck zu einem, der keine
Werte wegnimmt — und eine Regel, die mit *die Werte sind ohnehin weg* begründet
war, blieb wahr, nachdem ihr Grund aufgehört hatte zu gelten. Wenn Sie ändern,
was eine Geste tut, lesen Sie die Regeln nach, die mit dem begründet waren, was
sie vorher tat.

### D3 gilt auch für die Testsuite

Eine Geste ist ein Command hinaus und ein Delta zurück. Ein Test, der klickt und
dann liest, ist ein Test, der zu früh liest. Drei End-to-End-Flakes in diesem
Repository waren alle das, in drei Gestalten — und die schärfste ist, dass
`count()` nicht wartet: ein Wächter wie `if (count > 0) click()` macht aus einem
fehlenden Element einen **stillschweigend übersprungenen Schritt** und aus einer
kaputten Oberfläche einen grünen Lauf.

Warten Sie auf den Zustand, nie auf eine Dauer.

### Eine Messung mit einer anderen Regel als der des Codes ist keine Messung des Codes

Zwei aus einem Wegwerf-Skript gemeldete Zahlen waren beide falsch, und der
Corpus-Test überführte seinen eigenen Autor. Wenn Sie eine Zahl nennen, nennen
Sie eine, die der Code erzeugt hat.

---

## 4. Ein Command hinzufügen

Ein Command ist der einzige Weg, auf dem sich etwas ändert. Der Pfad, der Reihe
nach:

1. **Entscheiden, auf welche der drei Arten es wirkt** — Show, Session oder
   Maschine. Ist die ehrliche Antwort *keine davon*, halten Sie an und lesen Sie
   den `Shutdown`-Eintrag in `PROGRESS.md` §7, bevor Sie eine vierte erfinden.
2. **Die Variante deklarieren** in `prism_domain::Command`, mit einem
   Doc-Kommentar, der sagt, was sie tut *und warum sie diese Form hat*.
   Feldnamen sind auf der Leitung `camelCase`; das Enum ist intern mit `t`
   getaggt.
3. **Klassifizieren.** `Command::is_session_command`, `is_machine_command` und
   `is_undoable` sind drei getrennte Listen, und ein Command, das in einer fehlt,
   landet beim falschen Applier oder verschwindet aus dem Oops-Journal.
4. **In genau dem Applier implementieren, dem es gehört** —
   `prism_core::Show::apply`, `SessionState::apply` oder `MachineConfig::apply` —
   und die Deltas senden, die es verursacht. Ändert es ein gespiegeltes Feld,
   **prüfen Sie den handgeschriebenen Diff** (§3).
5. **Ihm ein Wort auf der Kommandozeile geben**, wenn ein Operator es tippen
   können soll: `prism_core::console`, `VERB_WORDS` und `CONSOLE_WORDS`. Ein Wort
   zur Grammatik hinzuzufügen, ohne es in die Tabelle zu setzen, ist genau das,
   was `the_verb_table_partitions_every_word_the_console_knows` verhindern soll —
   und die Wortliste des Handbuchs wird von
   `crates/prism-core/tests/documentation.rs` neu erzeugt.
6. **Die Tests vor dem Code schreiben**, nach `CLAUDE.md`. Mindestens: angewandt
   auf eine gefüllte Show, abgelehnt von den beiden Appliern, denen es nicht
   gehört (mit dem Zustand danach byte-identisch geprüft), und sein Platz in der
   Zählung in `command_application.rs`.
7. **Die Form auf der Leitung dokumentieren** in
   [`docs/IPC_PROTOCOL.md`](../IPC_PROTOCOL.md) §5.
8. `cargo test -p prism-domain`, um das TypeScript neu zu erzeugen.

---

## 5. Einen Fenstertyp hinzufügen

1. **Die Variante hinzufügen** zu `prism_domain::WindowType` **und zu
   `WindowType::ALL`**, das der Wähler des Control-Editors, die
   Proptest-Strategie und drei Tests ablaufen.
2. **Den Fall hinzufügen** in `ui/src/canvas/content.tsx`. Ein Fenster, das
   dieser Build nicht zeichnen kann, wird absichtlich *von der Leinwand
   weggelassen* statt als leerer Rahmen gezeichnet — ein fehlender Fall ist also
   lautlos, und der Switch ist das, was ihn nicht lautlos macht.
3. **Die Komponente schreiben.** Sie scrollt **in ihrem eigenen Körper** und nie
   außerhalb der Leinwand (`CLAUDE.md`). Sie liest über `ui/src/mirror/` und hält
   nichts: was sie zeichnet, muss überleben, geschlossen und wieder geöffnet zu
   werden, und muss auf einem zweiten Bildschirm dasselbe sein.
4. **Ist es noch nicht gebaut, sagen Sie es im Fenster.** Zwei der vierzehn tun
   genau das, mit Absicht: wer eines von einer X-Touch-F-Taste öffnet, soll eine
   Antwort finden und nicht ein leeres Rechteck, über das er einen Fehler
   meldet.
5. **Sein Kapitel schreiben** in [`operator.de.md`](operator.de.md) — ein
   Abschnitt, überschrieben mit dem Variantennamen, und eine Zeile in der
   erzeugten Tabelle. Der Test schreibt die Tabelle mit einem `TODO` neu und
   scheitert, bis beides existiert.
6. `cargo test -p prism-domain` für die Bindings, dann
   `npm --prefix ui run test`.

---

## 6. Einen Fixture-Typ hinzufügen — oder dem Bibliotheks-Reader etwas beibringen

Meistens lautet die Antwort **nicht** „ein neuer `AttributeType`". Lesen Sie in
dieser Reihenfolge:

**Ist es eine Beschriftung, die ein Operator liest, oder ein Schlüssel, unter dem
ein Preset abgelegt wird?** Eine Beschriftung kommt aus der Fixture-Datei —
`AttributeDef::label` trägt den Kanalnamen des Herstellers auf den Encoder. Ein
Schlüssel darf das **nicht**, denn `1 gobo at 50` muss den Kopf erreichen, dessen
Datei *Gobo* sagt, und den, dessen Datei *Gobo Wheel* sagt, gleichermaßen. Die
Capability-Typen der Open Fixture Library sind eine **geschlossene Menge**, es
gibt also ohnehin keinen offenen Namen zum Übernehmen.

**Nennt das Format eine Unterscheidung, die das Modell nicht hat?** Dann wächst
das Enum, und es wächst durch **Anhängen** — von den ersten fünfzehn Zeilen ist
zugesichert, dass sie dort stehen, wo sie stehen, denn das ist es, was *Rot,
Grün, Blau, Weiß* zur ersten Seite der Farbbank macht. `AttributeType::ALL` hat
heute 41 Zeilen.

**Ist es ein Kanal, den der Reader nicht versteht?** Dann ist er schon behandelt:
jeder DMX-Slot, der kein anderes Attribut erreicht, erreicht
`AttributeType::Raw`, auf der Control-Bank, unter dem Namen des Herstellers oder
als `Ch 7`. Fügen Sie keinen Sonderfall hinzu; wenn `Raw` eine schlechte Antwort
gibt, sitzt die Behebung im Reader.

Wenn das Enum doch wächst:

1. Die Variante anhängen und sie auf eine `FeatureGroup` legen.
2. Ihren **Ruhewert** entscheiden: `is_additive_emitter` entscheidet offen gegen
   geschlossen, und ein Rad ist keines von beidem (sein Wert ist eine
   Slot-Nummer). Das falsch zu machen lässt einen Rig beleuchtet hochkommen —
   siehe §3.
3. Sie der Zuordnung in `prism_core::library::ofl` hinzufügen.
4. **Den Corpus laufen lassen.**
   `every_channel_in_the_installed_library_maps_to_an_attribute`,
   `no_slot_of_any_profile_is_out_of_reach`,
   `no_profile_in_the_installed_library_rests_a_colour_shut` und
   `every_profile_in_the_installed_library_is_one_a_show_accepts` sind die vier,
   die finden, was ein handgeschriebener Test nicht kann. Sie brauchen die
   installierte Bibliothek.
5. Prüfen, ob der Tick weiterhin nichts alloziert — und ob die neue Form einen
   elften Pfad in `tick_allocations.rs` verdient.

---

## 7. Eine Ausgangsart hinzufügen

Die Naht ist `prism_protocols::DmxOutput` — drei Methoden — und sie wurde so
gewählt, dass die **Tests** sie implementieren können.

1. **Das Paket von Hand schreiben, gegen die Spezifikation, Feld für Feld.**
   Beide Netzprotokolle hier sind so entstanden: die Naht, die ein externes Crate
   bräuchte, ist größer als das Paket, und verlangt ist eine Byte-für-Byte-Zusage
   gegen einen veröffentlichten Standard — der man am leichtesten traut, wenn die
   Bytes einmal neben den Feldnamen stehen.
2. **Den Socket hinter ein Trait legen.** `UdpSender` und `FtdiBackend` sind die
   beiden, die es gibt. Das erlaubt, das Paket ohne jedes Netz zu prüfen *und* es
   noch einmal an einem über Loopback empfangenen Datagramm zu prüfen.
3. **Die Variante hinzufügen** zu `prism_domain::OutputKind`, mit ihren
   Parametern, und die Zeile im Einstellungsfenster, die sie bearbeitet.
4. **Die drei Dinge entscheiden, die jeder Treiber beantworten muss:**
   - was seine `health` bedeutet, und ob *der Socket hat es angenommen* ehrlich
     ist (bei einem Netzprotokoll ist es das nicht — das ist Punchlist-Eintrag
     B6);
   - was ein **Refresh** kostet, und ob er eine Kadenz früher hinausgeht, damit
     der größte Abstand wirklich ein größter ist;
   - was beim Herunterfahren passiert.
5. **Wieder verbinden, nicht scheitern.** Ein mitten in der Show gezogenes Kabel
   ist der erwartete Fall: `catch_unwind`, exponentielles Zurückweichen, die
   Lampe geht rot, die Engine läuft weiter.
6. **Nie broadcasten, und nie Multicast aus einem Test senden.** Eine Suite, die
   Lichtdaten auf das Netz legt, in dem sie läuft, tut einem Build-Server das an,
   was ein Broadcast einer Schule antut.
7. Was nur echte Hardware beantworten kann, als Zeile in
   `ARCHITECTURE_SPEC.md` §14 festhalten — **mit einem Rezept, dem jemand ohne
   dieses Repository folgen könnte** — und die gerätespezifischen Tatsachen als
   *Daten* halten, damit ihre Überprüfung eine Datenaktualisierung und ein Test
   ist und kein Refactoring.

---

## 8. Die Gates, und wofür jedes da ist

Alle davon sind auf jedem Commit grün, und ein Pull Request ist nicht fertig,
bevor sie es sind. **Lassen Sie die Rust- und die Oberflächen-Suite nacheinander
laufen** — beide gleichzeitig lässt Browser-Tests an Timeouts scheitern, die
allein durchgehen.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
npm --prefix ui run lint
npm --prefix ui run test
npm --prefix ui run build
npm --prefix ui run e2e
```

und der Typecheck, der **aus `ui/` heraus** laufen muss, weil `tsc -b` seine
Solution-Datei relativ zum Arbeitsverzeichnis auflöst:

```bash
cd ui && npx tsc -b --force
```

`-b`, nicht `--noEmit`: `ui/tsconfig.json` ist eine Solution-Datei ohne eigene
Dateien, `--noEmit` darauf prüft also überhaupt nichts. `--force`, weil eine
zwischengespeicherte Build-Info einen alten Fehler durchlassen würde.

Die CI fährt neun Jobs, und jeder ist aus einem Grund da, den man kennen
sollte:

| Job | Wofür er da ist |
|---|---|
| **Windows — voller Build und Test** | Das Zielsystem. Alles, inklusive Corpus |
| **Windows — Shell und Installer** | Ein Installer, der nur auf dem Rechner eines Einzelnen baut, ist eine Datei und kein Release. Der Job *untersucht* die erzeugte `.exe` außerdem, weil ein Bundler, dem man eine Konfiguration ohne Nutzlast gibt, fröhlich mit null endet |
| **Linux — plattformneutrale Crates** | Die Crates, die kein `#[cfg(target_os)]` enthalten dürfen, laufen auf einer zweiten Plattform. Das ist es, was die Behauptung belegt |
| **Linux ARM64 — Cross-Compile-Prüfung** | Fängt Plattformcode, der in ein neutrales Crate sickert, sodass ein Portabilitätsbruch an dem Commit scheitert, der ihn verursacht hat, und nicht ein Jahr später |
| **UI — Typecheck, Lint, Test, Build** | Mit den aus Rust neu erzeugten Bindings zuerst, damit eine veraltete Bindung nicht durchgehen kann |
| **UI — End-to-End gegen einen Daemon** | Ein echter `prismd`, ein echtes Chromium, und ein Daemon, den die Spezifikation unter dem Browser *tötet* |
| **Web — die Dokumentationsseite** | Eine Seite, die nur auf einem Rechner baut, ist dasselbe Problem wie ein Installer, der das tut |
| **Web — die Startseite** | Dasselbe Argument für `site/`. Ein eigener Job und kein Schritt im vorigen, weil die beiden Crates Entgegengesetztes versprechen: von der Dokumentationsseite ist zugesichert, dass sie **kein** Skript trägt, die Startseite trägt eines mit Absicht |
| **Web — der Auslieferungs-Container** | `deploy/` ist der selbstgehostete Weg, und ein Image, das nur auf der Maschine baut, auf der es ausgeliefert wird, ist die dritte Gestalt desselben Problems. Er stellt außerdem die eine Frage, für die es die beiden vhosts gibt: unterscheidet derselbe Port `prismdmx.de` und `docs.prismdmx.de`, und bekommt ein unbekannter Host keines von beiden |

Das Doc-Gate ist eine Anmerkung wert: `cargo doc` läuft mit verbotenen Warnungen,
ein kaputter Intra-Doc-Link lässt den Build also scheitern. Die
Crate-Dokumentation ist ein großer Teil dessen, was dieses Projekt weiß, und ein
Link, der lautlos aufgehört hat aufzulösen, ist ein Absatz, den niemand lesen
wird.

---

## 9. Testregeln, über die nicht verhandelt wird

Aus `CLAUDE.md`, plus dem, was die Praxis ergänzt hat:

- **Kein Test darf ein Gerät brauchen.** Die ganze Suite läuft auf einem Laptop,
  an dem nichts steckt. Ein Werkzeug, das Hardware braucht, lebt *außerhalb* des
  Workspace, und was es erzeugt hat, kommt als aufgezeichnete Fixture zurück, die
  die gewöhnliche Suite abspielt.
- **Kein Test darf ein Fenster brauchen.** Die Entscheidungen der Shell sind
  Funktionen, die ein Test ohne jedes Tauri aufruft. Was wirklich der Plattform
  überlassen bleibt, ist eine Zeile in §14 mit einem Rezept.
- **Kein Test sendet Multicast, und keiner broadcastet.**
- **Testgetrieben für Kernlogik.** Was Programmer, Cue-Engine, Merge oder
  Surface-Übersetzung berührt, kommt mit seinen Tests an.
- **Abdeckung:** ≥ 85 % global, > 95 % auf Engine, Programmer und Protokollen.
  Eine Abdeckungsmessung ohne `cargo llvm-cov clean --workspace` davor — und
  zwischen zwei Crates — ist keine Messung.
- **Eine auf einer ausgelasteten Maschine gemessene Leistungszahl ist keine
  Zahl.** Lassen Sie die nackte Tick-Kontrolle neben der vollen laufen; das Paar
  ist die Ablesung, nie die erste Zahl allein.
- **Auf Zustand warten, nicht auf eine Dauer** (§3).

---

## 10. Wie Arbeit festgehalten wird

Drei Dateien, und sie sind nicht austauschbar:

| | |
|---|---|
| [`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) | Was jede Session ist, mit Deliverables und **Exit-Kriterien**. Session-Nummern sind Identität, nie Reihenfolge; die Laufreihenfolge ist eine Tabelle am Ende |
| [`PROGRESS.md`](../../PROGRESS.md) | Was **gemessen** wurde, nicht was beabsichtigt war. §2 ist ein Verifikationsprotokoll je Session, §5 die offenen Verifikationen, §6 das Entscheidungsprotokoll, §7 die weitergetragenen Lehren, §8 der Prompt, der die nächste Session startet |
| [`docs/ISSUES.md`](../ISSUES.md) | Was bekanntermaßen falsch ist, auf Deutsch, nummeriert `Bnn`. Nummern sind Identität und werden nie neu vergeben; ein Eintrag wird mit ✅, ⛔ oder ➡️ geschlossen und **nie durch Löschen** |

Commits folgen Conventional Commits: `feat(engine):`, `fix(mcu):`,
`test(programmer):`, `docs(arch):`.

**Markdown wird im selben Durchgang aktualisiert wie der Code**, nicht in einem
Folge-Commit. Das ist eine ausdrückliche Anweisung des Eigentümers, und es ist
der Grund, warum ein Handbuchkapitel und der Code, den es beschreibt, nicht einen
Commit auseinanderliegen können.

---

## Wie es weitergeht

| | |
|---|---|
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) | Warum es so gebaut ist |
| `cargo doc --workspace --no-deps --open` | Die Crate-Dokumentation, die ungewöhnlich vollständig ist und die Spezifikation jedes Typs darstellt |
| [`../DMX_MERGE.md`](../DMX_MERGE.md) · [`../IPC_PROTOCOL.md`](../IPC_PROTOCOL.md) · [`../MCU_MAPPING.md`](../MCU_MAPPING.md) · [`../COMMAND_LINE.md`](../COMMAND_LINE.md) | Die vier Referenzen |
| [`../../CLAUDE.md`](../../CLAUDE.md) | Die Standards, an die sich dieses Projekt hält |
