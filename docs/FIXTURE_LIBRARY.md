# Woher ein Pult seine Fixture-Bibliothek bekommt

**Der Stand nach S61 und die Rechtslage dazu, recherchiert am 2026-09-21.**

Diese Datei ist der eine Ort, an dem steht, *warum* die Bibliothek so ausgeliefert
wird, wie sie ausgeliefert wird. `profiles/fixtures/SOURCE.md` sagt, **wie** man
sie installiert; hier steht, **warum es nicht einfacher geht**.

---

## 1. Die Ausgangslage

Seit S61 liest das Pult zwei Formate:

| | GDTF | Open Fixture Library |
|---|---|---|
| Was es beschreibt | das **Gerät** — Kanäle, Gobo-Bilder, 3D-Modell, Maße, wo der Strahl austritt | die **Kanäle** — was auf welchem Slot liegt und wie es heißt |
| Wer es schreibt | der **Hersteller** | Freiwillige |
| Wer es besitzt | der **Hersteller** | die Beitragenden, unter MIT |
| Woher | gdtf-share.com, **Konto nötig** | GitHub, frei |
| Für S30 brauchbar | **ja** | nein — es gibt nichts zu zeichnen |

GDTF ist der Grund, warum S61 überhaupt lief: der 3D-Viewer kann ein Gerät nicht
zeichnen, von dem er nur die Kanalliste kennt.

---

## 2. Warum der Installer keine GDTF-Bibliothek mitliefert

Kurz: **wir dürfen nicht.** Es sind zwei Probleme, und das zweite ist das
größere.

### 2.1 Die Nutzungsbedingungen von GDTF Share

Wörtlich aus den [Terms and
Conditions](https://gdtf-share.com/landing/pages/termsAndConditions.php):

> „You must not use any part of the materials on our Website for commercial
> purposes without obtaining a license to do so from us or our licensors."

Dazu: Inhalte dürfen nicht verändert oder zur Entwicklung von Ergänzungen
verwendet werden, die Schutzrechte verletzen könnten; bei einem Verstoß erlischt
das Nutzungsrecht sofort und angefertigte Kopien sind zu vernichten
([Terms of Use](https://gdtf-share.com/help/terms_gdtf.html)).

„Commercial purposes" ist unbestimmt. PrismDMX ist kostenlos und MIT-lizenziert,
aber darauf eine Release-Pipeline zu bauen, die bei jedem Tag tausende fremde
Dateien mitverpackt, wäre eine Wette auf die Auslegung eines Begriffs.

### 2.2 Die Dateien gehören GDTF Share gar nicht

Das ist der Punkt, der auch durch eine Erlaubnis von GDTF Share nicht
verschwindet. GDTF ist ausdrücklich so gebaut, dass die **Hersteller** ihre
Dateien behalten:

> „The value to manufacturers in creating GDTF files is in retaining ownership of
> writing the fixture file and its details, rather than a third-party developer."
> — [Vectorworks/PLSN über
> GDTF](https://blog.vectorworks.net/gdtf-finding-common-ground-for-an-efficient-workflow)

Ein Bündel bräuchte also nicht eine Erlaubnis, sondern **viele** — eine pro
Hersteller. Das **Format** ist offen; die **Dateien** sind es nicht.

### 2.3 Was daraus folgt

**Kein Bündeln ohne schriftliche Erlaubnis.** Auch nicht über ein
CI-Secret — technisch wären das sechs Zeilen YAML, und genau deshalb steht der
Grund hier aufgeschrieben, damit ihn nicht in einem halben Jahr jemand für ein
Versehen hält und „repariert".

> **Kein Rechtsrat.** Das oben ist eine Zusammenfassung dessen, was öffentlich
> nachzulesen ist. Wer Gewissheit will, fragt GDTF Share (betrieben von MA
> Lighting) schriftlich. Das kostet eine E-Mail und ist die einzige Antwort, die
> Bestand hat.

---

## 3. Der Weg, den das Format selbst vorsieht

Es gibt eine **offizielle öffentliche API**, dokumentiert von den GDTF/MVR-
Entwicklern ([mvrdevelopment/tools](https://github.com/mvrdevelopment/tools/blob/main/GDTF_Share_API/GDTF%20Share%20API.md)),
mit dieser Begründung:

> „provided for the convenience of users who want to integrate the GDTF Share
> into their own applications"

Anwendungsentwickler und Konsolenhersteller sind als Zielgruppe benannt. Ein
**Konto ist erforderlich**; die Sitzung läuft über ein Cookie mit zwei Stunden
Gültigkeit.

| | |
|---|---|
| Basis | `https://gdtf-share.com/apis/public/` |
| `login.php` | POST, `user` + `password` |
| `getList.php` | GET, alle veröffentlichten Fixtures mit ihren `rid` |
| `downloadFile.php?rid=N` | GET, eine `.gdtf` |

> **Eine Falle in der offiziellen Doku:** sie nennt `Content-Type:
> application/json`, die funktionierende Referenzimplementierung sendet aber
> **form-encoded** (`session.post(url, data=data)` in
> [open-stage/blender-dmx](https://github.com/open-stage/blender-dmx)).
> `tools/fetch-fixtures/*` sendet ebenfalls form-encoded — also so, wie es
> nachweislich funktioniert. **Ungetestet gegen den echten Dienst**, weil der
> Entwicklungscontainer `gdtf-share.com` nicht erreicht.

### Der Präzedenzfall

**BlenderDMX** — GPLv3, quelloffen, kostenlos — macht genau das: der Nutzer legt
sich ein eigenes Konto an und trägt die Zugangsdaten in den Addon-Einstellungen
ein; gebündelt wird nichts
([Doku](https://blenderdmx.eu/docs/gdtffixture/)). *Glowstone* ebenso, mit
lokalem Cache ([Doku](https://glowstone.build/docs/library/gdtf-share)). Es war
**kein** Produkt zu finden, das GDTF-Share-Inhalte im Installer mitliefert.

Der entscheidende Unterschied: lädt der **Betreiber** unter seinem eigenen
Konto, ist es seine Nutzung unter den Bedingungen, die er akzeptiert hat. Laden
**wir** und verteilen weiter, ist es eine Weitergabe.

---

## 4. Die Open Fixture Library dürfen wir mitliefern

Die [`LICENSE`](https://github.com/OpenLightingProject/open-fixture-library/blob/master/LICENSE)
des Repositories ist **MIT**, © 2017 Florian & Felix Edelmann, und deckt das
Repository einschließlich `fixtures/` ab. Weitergabe und kommerzielle Nutzung
sind erlaubt, solange Lizenztext und Copyright-Hinweis mitreisen —
`tools/fetch-fixtures/fetch-ofl.*` kopiert `LICENSE` deshalb mit.

**Das ist die Alternative für alle, die sich nicht einloggen wollen, und sie
funktioniert schon:** `release.yml` installiert den Corpus nach
`profiles\fixtures\ofl\`, und der Installer packt `profiles\fixtures\` ein. Ein
per Tag gebauter Installer trägt also **634 Fixtures ohne Konto und ohne Netz**
— nur eben ohne Gobo-Bilder, Modelle und Beam-Geometrie.

**Diese Zusage steht**, und der In-App-Login aus S62 hat sie nicht abgelöst: ein
Pult ohne Konto muss eine brauchbare Bibliothek haben, und hat sie. Der Login
ist ein Abschnitt in den Einstellungen, keine Tür.

---

## 5. Die vier Wege, auf denen eine Bibliothek auf ein Pult kommt

| | Konto? | Netz? | Status |
|---|---|---|---|
| **OFL im Installer** | nein | nein | **da** — `release.yml` + `fetch-ofl` |
| **Eigene Datei im Datenverzeichnis** | nein | nein | **da** — `.gdtf` oder `.json` in `fixtures\`, gewinnt gegen die installierte (B43); oder der Knopf *Import profile (GDTF)*, der die Datei genau dorthin kopiert |
| **MVR-Import** | nein | nein | **da** — Knopf *Import rig (MVR)* im Patch-Fenster, und eine `.mvr` im eigenen Ordner füllt auch ohne ihn die Bibliothek |
| **In-App-Login zu GDTF Share** | ja | ja | **da** — *Settings → This machine → GDTF Share*; ungetestet gegen den echten Dienst, siehe §3 |

Dazu der Pfad in *Settings → This machine → Fixture library*, mit dem ein
Betreiber auf ein Netzlaufwerk oder einen Stick zeigen kann.

### Warum MVR der wichtigste der vier ist

Eine `.mvr` ist ein ZIP, das **die GDTF-Dateien genau des Rigs enthält**, um das
es geht — es ist das Format, in dem ein Betreiber sein Rig vom Planer bekommt.
Ein MVR-Import löst das Bibliotheksproblem für den realen Fall vollständig, ohne
Konto, ohne Netz und ohne eine einzige Lizenzfrage: der Betreiber hat die Datei
bereits, sie gehört zu seiner Produktion.

Und der Leser dafür steht schon: `prism_core::library::zip` aus S61 liest das
Containerformat, `library::gdtf` liest, was darin liegt.

**Der erste Teil ist gebaut** (`prism_core::library::mvr`): eine `.mvr` in
`%APPDATA%\PrismDMX\fixtures\` wird beim Start gelesen wie eine `.gdtf` —
jedes Profil darin landet in der Bibliothek, unter dem Schlüssel, den das
**Profil** nennt, und gewinnt als eigenes gegen die installierte Bibliothek
(B43). Ein Fixture, das zweimal ankommt — einmal im Plan, einmal einzeln —, ist
**eine** Zeile.

**Und der Patch kommt mit** — die Entscheidung des Eigentümers vom 2026-09-21.
`Command::ImportRig` nimmt eine `.mvr` in die geöffnete Show: jedes Profil wird
eingebettet, jedes geplante Fixture gepatcht, **in einem Schritt, den ein Oops
ganz zurücknimmt** (die Regel aus S57, über eine Datei).

Drei Regeln, die dabei entschieden wurden:

- **Es wird hinzugefügt, nicht ersetzt.** Ein Fixture, das die Show schon hat,
  behält seine Nummer, seine Adresse und sein Profil. Das geplante Fixture
  bekommt die nächste freie Nummer und wird als *umnummeriert* gezählt. Die
  Nummerierung des Planers trifft auf die des Betreibers, und der Betreiber
  gewinnt.
- **Unvollständiges wird gezählt, nicht geraten.** Ein geplantes Fixture ohne
  Profil im Archiv, ohne Adresse, oder das nicht in sein Universe passt, wird
  übersprungen und gezählt.
- **Was passiert ist, wird gesagt.** Der Daemon schickt eine Notice, die auch
  nennt, was *nicht* ging — ein Import, der elf von zwanzig Fixtures gepatcht
  hat, muss das sagen.

---

## 6. Was S30 davon wissen muss

- Ein **GDTF-Profil** trägt `FixtureType::physical`: Maße, Modellname, jeden
  Beam mit Position (in Metern) und Richtung (Einheitsvektor, bei Grundstellung
  senkrecht nach unten) — und seit **S30b** das ganze Gerät
  (`prism_domain::device`): den Geometriebaum mit der Matrix jedes Knotens, jede
  Funktion jedes Kanals mit ihren Sets und Mode-Mastern und jedes Rad, das der
  Modus benutzt, mit Farbe (CIE xyY nach sRGB), Transmission, Bildname und
  Prismenfacetten jedes Slots.
- Ein **OFL-Profil und die vier Generics** tragen `None`. Das ist kein Fehler,
  sondern die Aussage *dieses Format beschreibt kein Gerät* — der Viewer macht
  daraus einen Moving Head oder eine PAR-Kanne, je nachdem, ob es Pan oder Tilt
  hat.
- **Die Modelle und Gobo-Bilder holt sich der Viewer selbst** (S30b): das Profil
  trägt nur die **Namen**, die GDTF den Dateien im Archiv gibt — mit Absicht,
  weil eine Show ihre Profile einbettet und ein Pfad in dieses Datenverzeichnis
  anderswo ins Leere zeigt —, und `Query::FixtureResource` liefert die Datei aus
  dem Archiv, das die Bibliothek unter der GUID des Geräts kennt
  (`docs/IPC_PROTOCOL.md` §5.2).
- **Die Matrix einer GDTF-Geometrie** ist die der Spezifikation: vier Zeilen,
  die **Verschiebung in der vierten Spalte, in Metern**. S61 hatte sie als
  Spalten gelesen und in Millimetern; ein veröffentlichtes Robe-Robin-T1-Profil
  hat das am 2026-09-21 widerlegt (S30b), und das `MATRIX_TO_METRES` in
  `library::gdtf::geometry` gibt es nicht mehr. Ein MVR schreibt seine Matrizen anders — `{u}{v}{w}{o}`, Millimeter —,
  und das ist in `library::mvr` richtig.
- Zum Ausprobieren ohne Konto: `docs/RELEASE_TEST_0.9.3.md` §1 baut mit
  Bordmitteln eine gültige `.gdtf`, und `ui/e2e/gdtf.ts` tut dasselbe in
  TypeScript.

---

## 7. Was S62 gebaut hat, und was daran offen bleibt

`IMPLEMENTATION_PLAN.md` trägt den Eintrag. Alle vier Punkte stehen:

1. ~~**MVR-Import**~~ — **fertig.** Der Leser (`library::mvr`), der Import mit
   Patch (`ShowFile::import_rig`) und der Knopf im Patch-Fenster.
2. ~~**`.gdtf`-Import**~~ — **fertig.** `Command::ImportProfile` kopiert die
   einzelne Datei von der Herstellerseite über den Dateidialog nach
   `fixtures\`, liest sie **vorher** und weigert sich, etwas abzulegen, das
   keine Fixture ist.
3. ~~**In-App-Login**~~ — **fertig.** *Settings → This machine → GDTF Share*:
   Benutzername, Passwort, ein Haken *auf diesem Rechner behalten*, eine
   Fortschrittszeile. Der Download läuft in einem eigenen Thread, das Pult
   spielt weiter, und `Delta::LibraryUpdate` sagt, wie weit er ist.
4. **OFL bleibt** die Grundausstattung im Installer.

Nicht geplant: bündeln. Siehe §2 und Entscheidung **D12**.

### Die zwei offenen Hälften

- **Der echte Dienst ist ungetestet.** Der Entwicklungscontainer erreicht
  `gdtf-share.com` nicht, und `CLAUDE.md` sagt, dass ein Test kein Gerät —
  und damit auch kein Netz — brauchen darf. Was geprüft ist, ist alles, was
  *entscheidet*: `prismd::share::update` gegen ein `Share` aus dem Speicher,
  der Antwortleser `parse_list` gegen echten Antworttext, und der ganze Weg
  Kommando → Thread → Fortschritt → Delta → Browser. Was **nicht** geprüft
  ist, sind die drei URLs und der Cookie-Jar. Der erste echte Login gehört auf
  die Liste in `docs/RELEASE_TEST_0.9.3.md`.
- **Passwörter merken geht nur auf Windows.** `crates/prismd/src/secrets.rs`
  nutzt den Windows-Anmeldeinformationsmanager. Auf Linux und dem Raspberry Pi
  hat ein Pult im Rack keine angemeldete Desktop-Sitzung, die einen Keyring
  aufschließen könnte, und ein stiller Rückfall auf eine Datei wäre genau der
  Klartext, den dieses Modul vermeiden soll. Dort wird das Merken **in Worten
  abgelehnt** und der Betreiber tippt sein Passwort beim Aktualisieren — was
  funktioniert.
