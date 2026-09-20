# Abnahmetest am Test-Rig — vor dem nächsten Release

**Gilt für:** den Build von `fix/b52-switching-channels` (enthält S56, S57 und
B52), Version im Installer weiterhin `0.9.2`.
**Rig:** der eigene Test-Rig, Show `default.prism`, `machine.json` wie auf dem
Entwicklungsrechner.

Jeder Punkt sagt **wo** (Fenster, Menü, Taste), **was zu tun ist** und **was
passieren muss**. Ein Kästchen, das nicht passiert, ist ein Befund: bitte mit
Nummer (z. B. *T-B59.3*) als GitHub-Issue oder in `docs/ISSUES.md` melden.

---

## 0. Vorbereitung

Das Rig laut Show und `machine.json`:

| Was | Wo |
|---|---|
| Fixture 1 — Stairville MH-z195 Smart White CW/WW/A, 15 ch | Universe 1, Adresse 1, über den **Art-Net-Node `2.16.10.66`** (Universes 1–4) |
| Fixture 2 — Lixada Mini Moving Head RGBW, 14 ch (= Stage Wash 7x10W) | Universe 5, Adresse 1, über **Open DMX** |
| Pult | Behringer X-Touch, MIDI-Port `X-Touch` |
| Show | Sequence 1 *Test* auf Executor 1 (Fader: Master), fünf Views: *Patch*, *Programmer*, *Settings*, *View 4* (Executors), *View 5* (leer) |

- [x] **T-0.1** Installer (`PrismDMX_0.9.2_x64-setup.exe`) auf dem Rig ausführen. Eine installierte 0.9.2 wird dabei **überschrieben** — gleiche Versionsnummer.
- [x] **T-0.2** Vor dem ersten Start nach `%APPDATA%\PrismDMX\` kopieren: `default.prism`, `machine.json` **und den Ordner `fixtures\`** (darin liegt das eigene Profil `stairville\mh-z195-smart-white-cw-ww-a.json` — die Show trägt es eingebettet, aber ohne den Ordner fehlt es in der Bibliothek).
- [x] **T-0.3** PrismDMX starten. Kopfzeile: grüner Punkt **Connected**. X-Touch-Scribble-Strips leuchten.
- [x] **T-0.4** Fenster *Settings* → Tab *Outputs*: der Art-Net-Output liest **OK**, sobald der Node antwortet (sonst *Degraded* mit dem Namen des Nodes — dann Kabel/Firewall prüfen, siehe B6 in `ISSUES.md`). Open DMX liest **OK**.

---

## 1. Neu seit S57 — das Patch-Fenster (B60, GitHub #28)

**Wo:** View **Patch** (in der View-Leiste oben) → Fenster *Patch*.

- [x] **T-B60.1 — *Add fixture* öffnet die Bibliothek.** Knopf *Add fixture* oben im Patch-Fenster. Es öffnet sich **ein** Fenster über der ganzen Arbeitsfläche: links die Bibliothek mit Suchfeld, rechts die Einstellungen (Count, Number, Name, Universe, Address). Kein zweites Formular unter der Tabelle.
- [x] **T-B60.2 — ein Eintrag pro Fixture.** Ins Suchfeld `stage wash` tippen. *Stage Wash 7x10W LED Moving Head* steht **einmal** da, Spalte *Modes*: `9ch · 14ch` (früher zwei Zeilen).
- [x] **T-B60.3 — die ganze Zeile wählt.** Auf die Spalte *Modes* oder *Fixture* dieser Zeile klicken (nicht auf den Herstellernamen). Rechts steht *Stage Right Stage Wash 7x10W LED Moving Head*, darunter ein Menü **Mode** mit `9ch` und `14ch`.
- [x] **T-B60.4 — Nachladen beim Scrollen.** Suchfeld leeren. Rechts oben neben dem Suchfeld steht *… of NNN fixtures*. In der Liste ganz nach unten scrollen: es kommen weitere Fixtures dazu (je 60), bis zum Ende der Bibliothek — ohne etwas zu tippen.
- [x] **T-B60.5 — ein neues Fixture beginnt an der nächsten freien Adresse.** *Stage Wash* wählen, **Universe auf `5` stellen**. Die Adresse springt von selbst auf **15** (Fixture 2 belegt 5.1–5.14), ohne dass man sie tippt. Mode auf `9ch` wechseln: die Adresse bleibt auf der nächsten freien passenden Stelle.
- [x] **T-B60.6 — ohne Namen heißt es wie sein Typ.** Das Feld *Name* leer lassen; grau steht darin *Stage Wash 7x10W LED Moving Head*. **Patch** drücken → neue Zeile heißt genau so.
- [x] **T-B60.7 — mehrere auf einmal, ein Oops.** *Add fixture* → *Stage Wash*, Mode `14ch`, **Count `3`**, Universe `5`. Die Zeile unter dem Formular sagt *3 fixtures, … at 5.x to 5.y*. **Patch** → drei Zeilen, nacheinander nummeriert, *… 1*, *… 2*, *… 3*, keine rot (keine Überlappung). Dann in der Kommandozeile `Oops` tippen + Enter (oder die Undo-Taste am X-Touch bei **leerer** Zeile): **alle drei** verschwinden auf einmal.
- [x] **T-B60.8 — die Überlappung nennt die nächste freie Adresse.** Zeile von **Fixture 2** in der Patch-Tabelle anklicken (dasselbe Fenster öffnet sich für dieses Fixture). Address auf `1` lassen, Universe auf `1` stellen. Unter dem Formular in Gelb: *Overlaps 1–… with fixture 1 — the higher fixture number wins. Next free: 1.16.* und ein Knopf **Move to 1.16**. Knopf drücken → Address = 16. Dann **Cancel** (nichts wurde gesendet — Fixture 2 steht weiter auf 5.1).
- [x] **T-B60.9 — ein Fixture bearbeiten.** Zeile von Fixture 1 anklicken: Mode-Menü zeigt `15ch` (und `12ch`). Namen ändern, **Apply** → die Tabelle zeigt den neuen Namen. Danach `Oops` → alter Name.
- [x] **T-B60.10 — Bildschirm.** Während das Bibliotheksfenster offen ist: die Seite selbst scrollt nirgends, nur die Liste darin.

## 2. Neu in diesem Build — Moduskanäle (B52)

Keines der beiden Rig-Fixtures hat einen Moduskanal, daher **virtuell**: ein
ADJ Flat Par QA12 auf **Universe 2** (geht an den Art-Net-Node, dort hängt nichts).

- [x] **T-B52.1** Patch → *Add fixture* → `flat par qa12` suchen → Zeile wählen → Mode **`8ch`**, Universe `2`, Address `1`, **Patch**. Die Nummer merken — unten **N** genannt (nach den Patch-Tests meist 4).
- [x] **T-B52.2** Kommandozeile: `Fixture N` + Enter. Im **Programmer-Band** unten die Bank **CONTROL** wählen. Darunter die Knöpfe *Strobe*, *Mode Select* und *Unused*.
- [x] **T-B52.3** Kommandozeile: `N control at 50` + Enter. Die Knöpfe heißen jetzt **Program Speed** und **Color Change Programs**.
- [x] **T-B52.4** `N control at 90` → **Sound Sensitivity** und **Sound Active Programs**. `N control at 0` → wieder **Strobe** / **Unused**.
- [x] **T-B52.5** Rechtsklick auf den Knopf *Color Change Programs* (bei `N control at 50`): die Liste zeigt die **Programme** (nicht die Makros des anderen Modus).
- [x] **T-B52.6 — über eine Cue.** Programmer leeren (**CLEAR** zweimal). `Fixture N`, `N control at 90`, dann `Store Cue 5` (Merge, wenn gefragt), Programmer leeren. `Goto Cue 5` + Enter: der Knopf heißt *Sound Sensitivity*, obwohl der Programmer nichts hält — er folgt dem, was am Kabel anliegt.
- [x] **T-B52.7** Aufräumen: `Oops` bis Cue 5 und Fixture N weg sind, oder `Delete Cue 5` und das Fixture im Patch mit **Unpatch** entfernen.

## 3. Neu seit S56 — die zehn Beta-Meldungen

### B55 — Controls-Menü stürzte beim Binden ab (#23, Blocker)
**Wo:** Fenster *Settings* → Tab **Controls**.
- [x] **T-B55.1** Im Abschnitt der eigenen Tasten **+** → Typ **Type a command**, Text `Go Executor 1`, **Learn**, am X-Touch eine freie Taste drücken. Der Client bleibt verbunden.
- [x] **T-B55.2** Dasselbe mit einer Zeile für **Choose a window** (Abschnitt *Other*): Learn, Taste drücken. Settings schließen und wieder öffnen, Tab Controls: **kein** Verbindungsabbruch, keine Rückkehr zu *Outputs*.
- [x] **T-B55.3** Die gebundenen Tasten drücken: *Choose a window* öffnet den Fensterwähler, *Go Executor 1* schreibt die Zeile.

### B54 — `fixtures/` fehlte nach frischer Installation (#9)
- [x] **T-B54.1** Nur prüfbar auf einer Maschine **ohne** `%APPDATA%\PrismDMX\`: nach dem ersten Start gibt es `%APPDATA%\PrismDMX\fixtures\README.txt`. (Auf dem Rig nach T-0.2 schon vorhanden — dann überspringen oder mit einem zweiten Windows-Benutzer testen.)

### B53 — Zahlenfelder ließen sich nicht leeren (#21)
- [x] **T-B53.1** **Wo:** Patch → Zeile anklicken. Im Feld *Address* alles löschen: das Feld bleibt leer, unten steht *Address has to be a whole number.*, **Apply** ist grau. `7` tippen → Apply wieder aktiv. Cancel.
- [x] **T-B53.2** **Wo:** *Settings* → **Outputs** → Art-Net-Output bearbeiten: *Number* und *Hop limit* lassen sich leeren und neu tippen; gespeichert wird erst beim Anwenden. Abbrechen.

### B56 — View-Wechsel löschte die Kommandozeile (#24)
- [x] **T-B56.1** Kommandozeile `Fixture 1 at` tippen (nicht Enter). Oben in der View-Leiste auf **View 4** klicken: die View wechselt, **die Zeile bleibt stehen**.

### B57 — erster Klick in ein nicht fokussiertes Fenster wählte nicht (#25)
- [x] **T-B57.1** View **Programmer**. Ins *Sequence Sheet* klicken (Fokus dort). Dann **einmal** auf Fixture 1 im *Fixture Sheet* klicken: das Fixture ist **sofort** gewählt (nicht erst beim zweiten Klick).

### B58 — Oops löscht Wörter (#26)
- [x] **T-B58.1** Kommandozeile `Fixture 1 at 50` tippen. Die **Oops**-Taste (Fenster *Command Keys*, über den Fensterwähler öffnen) drücken: nacheinander verschwinden `50`, `at`, `1`, `Fixture` — die Show ändert sich dabei nicht.
- [x] **T-B58.2** Dasselbe mit der **Undo-Taste am X-Touch**: Wort für Wort.
- [x] **T-B58.3** Bei **leerer** Zeile nimmt Oops/Undo die letzte Änderung zurück (z. B. einen Namen im Patch). `Oops` getippt + Enter ist immer ein Undo.

### B59 — Crossfade-Stellung nicht synchron (#27)
**Wo:** View **View 4** → Fenster *Executors* → Executor 1 anklicken → Editor rechts.
- [x] **T-B59.1** *Fader* auf **Crossfade** stellen. Die Beschriftung am Strip zeigt **XF**.
- [x] **T-B59.2** Den Motorfader von Executor 1 am X-Touch bis ganz oben schieben und **loslassen**: der Fader **bleibt oben** (fährt nicht auf 0 zurück). Im Browser steht der Fader von Executor 1 ebenfalls oben.
- [x] **T-B59.3** Im Browser den Fader von Executor 1 auf halbe Höhe ziehen: der Motorfader am X-Touch fährt auf halbe Höhe.
- [x] **T-B59.4** Mit einem zweiten Browser (`http://127.0.0.1:7373` oder die Desktop-App plus Browser) beide Bildschirme vergleichen: gleiche Stellung.
- [x] **T-B59.5** Oops nach einer Crossfade-Bewegung nimmt die **Stellung nicht** zurück. Zum Schluss *Fader* wieder auf **Master**.

### B61 — Rückmeldung verschob die Oberfläche (#29)
- [x] **T-B61.1** Eine View mit einem Fenster am unteren Rand öffnen. In der Kommandozeile tippen und wieder löschen, einen Befehl mit Fehler absenden (`Fixture 99` + Enter): die Arbeitsfläche **springt nicht** — die Rückmeldung steht in einer Zeile fester Höhe, lange Texte enden mit „…".

### B62 — Store-Leiste im Cue Viewer entfernt (#30)
- [x] **T-B62.1** View **Programmer** → *Cue Viewer*: **keine** Store-Leiste mehr.
- [x] **T-B62.2** Speichern über die Kommandozeile: Fixture 1 wählen, `at 50`, `Store Cue 3` → Cue 3 erscheint im Cue Viewer.
- [x] **T-B62.3** Cue 3 zum Bearbeiten öffnen (`Edit Cue 3`), einen Wert ändern: die **Update**-Taste im Fenster *Command Keys* **blinkt**. Update drücken → blinkt nicht mehr, Cue 3 hat den neuen Wert.

---

## 4. Rauchtest — was schon vorher ging

- [x] **T-S.1** Sequence *Test* mit **Go** (Executor 1, Taste *Go+* am X-Touch-Strip und im Browser) durchlaufen: beide Moving Heads reagieren, *DMX Sheet* zeigt Werte auf Universe 1 und 5.
- [x] **T-S.2** Stairville (Fixture 1): `1 at 100`, `1 pan at 50`, `1 warmwhite at 100` → Kopf fährt und leuchtet warmweiß. **CLEAR** zweimal → zurück.
- [x] **T-S.3** Lixada (Fixture 2, Open DMX): `2 at 100`, `2 red at 100` → rot. CLEAR.
- [x] **T-S.4** X-Touch: Encoder im Programmer-Band drehen, Bank-Tasten, Motorfader Executor 1 (Master) — alles reagiert.
- [x] **T-S.5** **Save** (Taste am X-Touch oder `Save`): LED geht aus. Programm schließen (Fenster schließen → läuft im Tray weiter), neu öffnen → alles da. Beenden über das Tray-Symbol.
- [x] **T-S.6** Nach dem Neustart des Rechners mit angeschlossenem X-Touch: Surface wird gefunden (*Settings* → *Devices*).

## 5. Wenn alles grün ist

Befunde melden, sonst: PR `fix/b52-switching-channels` → `master`, dann Tag
`v0.9.3` (oder die gewählte Version) — `release.yml` fährt den vollständigen
Durchlauf mit Windows und Installer.
