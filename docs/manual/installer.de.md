# PrismDMX — Handbuch für den Installateur

**Für:** wer das Pult in einem Haus einrichtet — Ausgänge, Netz, Maschine,
Autostart —, nicht für den, der damit eine Show fährt. Dafür gibt es das
[Operator-Handbuch](operator.de.md).
**Gilt für:** Version `0.9.2`.
**Sprache:** Deutsch. Alles, was das Programm auf den Bildschirm schreibt oder
auf der Kommandozeile heißt, steht englisch da, wo es englisch ist.

---

## Inhalt

1. [Was Sie einrichten](#1-was-sie-einrichten)
2. [Installation](#2-installation)
3. [Wo was liegt](#3-wo-was-liegt)
4. [Die Ausgänge](#4-die-ausgänge)
5. [Netz: sACN und Art-Net in einem Haus](#5-netz-sacn-und-art-net-in-einem-haus)
6. [Open DMX USB](#6-open-dmx-usb)
7. [Das Bedienpult](#7-das-bedienpult)
8. [Die Maschine: Netzwerkfreigabe, Token, Protokoll, Beenden](#8-die-maschine-netzwerkfreigabe-token-protokoll-beenden)
9. [Autostart](#9-autostart)
10. [Der Rechner ohne Bildschirm, und der Raspberry Pi](#10-der-rechner-ohne-bildschirm-und-der-raspberry-pi)
11. [Abnahme: was Sie prüfen, bevor Sie gehen](#11-abnahme-was-sie-prüfen-bevor-sie-gehen)
12. [Fehlersuche](#12-fehlersuche)

---

## 1. Was Sie einrichten

PrismDMX besteht aus **`prismd`** — dem Pult, das die Show hält und DMX
ausgibt — und einer **Oberfläche**, die nur zeichnet und schickt. Der Pult
prozess läuft weiter, wenn das Fenster zugeht, und das ist keine Bequemlichkeit,
sondern der Grund für die Aufteilung.

Als Installateur richten Sie drei Dinge ein, und sie gehören ausdrücklich
**nicht** in die Show-Datei:

| Was | Wo es liegt | Warum dort |
|---|---|---|
| Die Ausgänge — welche Universen wohin gehen | `machine.json` | Ein Rig ist eine Eigenschaft des **Gebäudes**. Eine Show, die auf einem Stick ins nächste Haus getragen wird, darf die Verkabelung des ersten nicht mitbringen |
| Die Identität dieses Pults (die sACN-CID) | `machine.json` | Aus demselben Grund, und weil ein Pult mit einer neuen CID bei jedem Start für jeden Empfänger eine **neue Quelle** ist |
| Der MIDI-Port des Bedienpults, das Protokoll, die Netzwerkfreigabe | `machine.json` | Das ist diese Maschine, nicht diese Show |

**Alles davon ist eine Einstellung im Fenster, kein Kommandozeilenschalter.**
Es gibt für jede auch einen Schalter — `prismd --help` listet sie —, aber ein
Schalter gilt **nur für diesen Lauf**: die gespeicherte Einstellung wird dann
weder gelesen noch geschrieben, und das Einstellungsfenster sagt, welcher
Schalter gerade welche Zeile festhält. Ein Haus richtet man im Fenster ein.

---

## 2. Installation

**Windows 10 oder 11, 64 Bit.** `PrismDMX_<version>_x64-setup.exe` von der
[Release-Seite](https://github.com/flakesystems/PrismDMX/releases).

**Es installiert pro Benutzer** und braucht keine Administratorrechte. Das ist
Absicht: ein Schullaptop, den der Operator nicht verwalten darf, ist der
Normalfall, und alles bis hin zum Autostart funktioniert ohne erhöhte Rechte.
Der Preis: die Installation gehört dem Benutzerkonto, unter dem sie lief. Wenn
mehrere Konten dasselbe Pult bedienen sollen, installieren Sie einmal je Konto.

**Es muss nichts danebeninstalliert werden.** Der gesamte Build ist gegen die
statische C-Laufzeitbibliothek gelinkt; es gibt kein Visual-C++-Redistributable
zu besorgen. Die eine Ausnahme ist die **Edge-WebView2-Laufzeit**, die das
Fenster zeichnet: Windows 11 hat sie, Windows 10 hat sie überall dort, wo Edge
aktualisiert wurde, und das Installationsprogramm holt sie bei Microsoft, wenn
sie fehlt. Das ist der einzige Schritt, der Internet braucht — in einem Haus
ohne Internet installieren Sie die WebView2-Laufzeit vorher von Hand.

**SmartScreen.** Der Build ist **nicht signiert**, also zeigt Windows beim
ersten Start *Der Computer wurde durch Windows geschützt*. Wenn Sie das nicht
per Klick übergehen wollen — eine vernünftige Haltung in einem fremden Haus —,
prüfen Sie stattdessen die Prüfsumme:

```powershell
Get-FileHash .\PrismDMX_0.9.2_x64-setup.exe -Algorithm SHA256
```

und vergleichen Sie sie mit der auf der Release-Seite. Die dortige Prüfsumme
wird von demselben Build-Lauf gebildet, der die Datei erzeugt hat; niemand tippt
sie ab.

**Aktualisieren** heißt: das neue Installationsprogramm laufen lassen. Es
ersetzt das Programm an Ort und Stelle, rührt Ihre Daten nicht an und hält ein
laufendes Pult nicht an — ein laufendes Pult ist allerdings so lange der alte
Build, bis es neu gestartet wurde.

---

## 3. Wo was liegt

| | Wo (Windows) |
|---|---|
| Das Programm, `PrismDMX.exe` und `prismd.exe` daneben | `%LOCALAPPDATA%\PrismDMX` |
| Shows, `machine.json`, eigene Fixture-Profile, Tastenbelegung, Protokoll | `%APPDATA%\PrismDMX` |
| Die mitgelieferte Fixture-Bibliothek | neben dem Programm, `profiles\fixtures` |

`%APPDATA%\PrismDMX` ist das **Datenverzeichnis**. Es ist die einzige
Einstellung, die gelesen und nie geschrieben wird, und der Grund ist, dass die
Einstellungen *darin* liegen: ein Pult, dem man sagt, es solle umziehen, müsste
das woanders erfahren. Wenn Sie es verlegen müssen, ist der Weg `--data-dir`
oder die Umgebungsvariable `PRISMD_DATA_DIR`, und das Einstellungsfenster nennt
das Verzeichnis, damit man `machine.json` findet.

**Deinstallieren entfernt das Programm und sonst nichts.** Das Datenverzeichnis
bleibt liegen, und eine Neuinstallation findet es wieder. Es gibt keine Version
von „ich will das neu installieren", die auch „wirf meine Show weg" heißt.

Zwei Dateien im Datenverzeichnis sind Betriebszustand und keine Konfiguration:

- **`prismd.guard`** — eine leere Datei, auf der der laufende Daemon eine
  Sperre des Betriebssystems hält. Sie ist der Grund, warum es nie zwei Pulte
  auf einem Rig gibt: die Frage ist *hält das jemand*, nicht *lebt Prozess 4711*,
  und das Betriebssystem gibt die Sperre auch dann frei, wenn der Prozess
  abgeschossen wurde. Eine übrig gebliebene Datei blockiert nichts.
- **`prismd.lock`** — daneben, lesbar, mit der Prozess-ID, den Endpunkten und
  dem Token. Daran findet ein Fenster sein Pult.

---

## 4. Die Ausgänge

*Settings → Outputs*. Ein Ausgang ist eine Zeile mit fünf Angaben:

| | |
|---|---|
| **Nummer und Name** | Der Name ist für Menschen. Ihn zu ändern kostet **nichts** — kein Neustart, kein Frame |
| **Art** | Art-Net, sACN oder Open DMX USB |
| **Universen** | Welche Universen *dieser* Ausgang trägt. Nicht alle |
| **Aktiv** | Ein deaktivierter Ausgang behält seine ganze Konfiguration und hat keinen Thread und keinen Socket — das, was man von einer Node will, an der gerade gearbeitet wird |

**Beides geht in beide Richtungen:** ein Universe darf auf mehrere Ausgänge
gehen, und ein Ausgang darf viele tragen. Ein Universe, das an zwei Ausgänge
geht, kommt bei beiden **byteidentisch** an.

**Ein gepatchtes Universe, das kein Ausgang trägt, ist ein zulässiger Zustand,
und er wird gemeldet.** Das Pult zu verbieten, in ein noch nicht verkabeltes
Universe zu patchen, hieße ein Pult, das man vor dem Einbau nicht vorbereiten
kann; es stillschweigend fallen zu lassen, ist der Weg, auf dem ein Universe
dunkel bleibt, während alle Lampen grün sind. *Settings → Outputs* nennt diese
Universen namentlich. **Lesen Sie diese Zeile bei der Abnahme.**

**Was einen Neustart des Ausgangs kostet:** die Art, die Universen, die Nummer,
der Aktiv-Schalter. Sonst nichts. Ausgänge, die sich nicht geändert haben,
kostet ein Umbau **keinen Frame und keinen Tick** — Sie dürfen im laufenden
Betrieb eine Node hinzufügen.

**Der Frame-Zähler** in der Zeile wird einmal pro Sekunde nachgefragt, solange
das Fenster offen ist. Er ist das Mittel, um zu sehen, ob eine gerade angelegte
Zeile wirklich etwas tut.

---

## 5. Netz: sACN und Art-Net in einem Haus

### Grundsätzliches

**Dieses Pult sendet nie einen Broadcast**, außer Sie verlangen es ausdrücklich
namentlich. Art-Net geht **unicast** an die Adressen, die Sie eintragen — ein
Art-Net-Broadcast flutet ein Schulnetz —, und sACN geht multicast an die
Gruppenadresse des Universes oder unicast an einen benannten Empfänger.

**Beide senden bei Änderung und sonst als Auffrischung.** 64 Universen mit 44 Hz
unverändert zu senden wären 1,5 MB/s Nichts.

### Art-Net

| Einstellung | Was Sie wissen müssen |
|---|---|
| **Ziel** | Host und Port. Der Port ist 6454, wenn Sie keinen nennen |
| **Port-Adresse** | Universe → Net · Sub-Net · Universe **auf dieser Node**. Voreinstellung: Universe *N* → Port-Adresse *N − 1*, denn PrismDMX zählt Universen ab 1 und Art-Net ab 0. Eine Vierfach-Node sind vier Zeilen, die Sie hinten am Gerät ablesen — **Nodes sind sich darüber nicht einig**, deshalb ist es einstellbar |
| **ArtSync** | Aus. Eine Node, die es versteht, zeigt nichts mehr an, bis eines kommt. Wenn Sie es einschalten, geht es an **dieselben** Adressen wie die Daten — eine Unicast-Konfiguration wird davon keine sendende |
| **Auffrischung** | Mindestens alle 800 ms, und eine Kadenz *früher*, damit die Lücke nicht 800 ms plus eine Kadenz wird |

**Node-Erkennung.** Das Pult schickt alle drei Sekunden ein `ArtPoll` an
**genau die Adressen, an die es ohnehin sendet** — nirgends sonst hin, und nie
als Broadcast. Was zurückkommt, steht in *Settings → Outputs* unter der Zeile:
Name der Node, IP, und ob sie *antwortet*, *nie geantwortet hat* oder
*aufgehört hat*, mit dem Alter der letzten Antwort daneben.

Das ist der Unterschied zwischen *das Socket hat das Datagramm genommen* und
*jemand hat es bekommen*. Ein Art-Net-Ausgang, dessen Nodes schweigen, liest
**Degraded** und nicht *OK*.

**Eine Node, deren Adresse niemand eingetragen hat**, wird nur gefunden, wenn
sie sich **selbst meldet** — das tun Nodes beim Einschalten und bei einer
Konfigurationsänderung, und das Pult hört auf Art-Nets eigenem Port zu.

> **Die Firewall ist der Fall, der in einem echten Haus schon zugeschlagen hat.**
> Die eingehende Regel deckte das Profil *Privat* ab, das Lichtnetz war *Öffentlich*,
> und jede Antwort wurde verworfen, bevor der Prozess sie sah. Das Pult sagt das
> inzwischen von selbst: gehen Polls hinaus und kommt gar nichts zurück, steht in
> *Settings → Outputs* ein Vorschlag, was zu prüfen ist. Prüfen Sie in dem Fall,
> welchem Netzwerkprofil Windows die Lichtnetzkarte zugeordnet hat.

### sACN (E1.31)

| Einstellung | Was Sie wissen müssen |
|---|---|
| **CID** | Die Identität dieses Pults, eine UUID, **Konfiguration**. Ein Ausgang ohne CID verbindet nicht, statt unter der Null-CID zu senden, die jedes nicht eingerichtete Pult teilen würde |
| **Adressierung** | Multicast `239.255.<Universe hoch>.<Universe niedrig>`, Port 5568. Unicast an benannte Empfänger für Häuser, die Multicast verbieten |
| **Universe-Nummern** | Universe *N* → E1.31-Universe *N*. Beide zählen ab 1 — trotzdem einstellbar, denn das Universe 1 eines Hauses ist nicht immer das Universe 1 eines Pults |
| **Priorität** | **Pro Universe**, 0…200, Voreinstellung 100. So übernimmt ein Ersatzpult: zwei Quellen auf einem Universe entscheidet die höhere Priorität. Über 200 wird abgelehnt, nicht gekappt |
| **Quellenname** | 64 Byte aus der Show-Datei, an einer Zeichengrenze gekürzt |
| **Hop-Limit (TTL)** | Wird beim Verbinden ausdrücklich gesetzt, Voreinstellung 1. Ein Hop-Limit, das nicht gesetzt werden konnte, macht den Ausgang **Disconnected** — sonst hätten Sie ein geroutetes Lichtnetz bestellt und bekämen Datagramme, die am ersten Router stehen bleiben, unter einer grünen Lampe |
| **Abschalten** | Drei `Stream_Terminated`-Pakete je aktivem Universe, jedes mit der **nächsten** Sequenznummer |

**Was ein Switch können muss:** IGMP-Snooping, sonst geht jedes Universe an
jeden Port. Wenn das Haus geroutet ist, muss das Hop-Limit hoch — es ist eine
Einstellung, kein Codeproblem.

**Zwei Sender auf einem Universe** sind der häufigste Fehler in einem Haus mit
zwei Pulten, und keiner der beiden merkt es von allein. Die Priorität ist das
Mittel dagegen, und sie gehört bei der Abnahme aufgeschrieben.

---

## 6. Open DMX USB

Der unterstützte Adapter ist ein **FTDI FT232R ohne Mikrocontroller** — geprüft
mit einem DSD TECH SH-RS09B (`0403:6001`, Produktzeichenkette
`FT232R USB UART`). Es gibt keine Widget-Firmware, die Break und
Mark-After-Break erzeugt, also macht der **Rechner** das DMX-Timing.

Drei Dinge, die Sie einem Haus sagen müssen:

1. **Genau ein Universe je Adapter.** Zwei wird abgelehnt, nicht abgeschnitten.
2. **Rund 35 Hz**, nicht 44. Gemessen: 35,5 Hz über 60 s. Das ist für diese
   Bauart normal — QLC+ und FreeStyler erreichen mit demselben Kabel nicht mehr
   — und für Dimmer und LED-PARs unproblematisch. Wer garantierte 44 Hz braucht,
   nimmt Art-Net oder sACN. Das Pult sagt das selbst, wenn der Ausgang angelegt
   wird.
3. **Zwei Adapter unterscheiden sich über die Seriennummer.** Die steht in der
   Ausgangs-Zeile. Ohne sie nimmt jeder Ausgang den erstbesten Adapter.

**Ein gezogenes Kabel ist der erwartete Fall**, nicht die Ausnahme: der Thread
erkennt den Fehler, räumt auf und verbindet neu, die Lampe wird rot und die
Engine läuft weiter.

**Unter Linux und auf dem Raspberry Pi** greift `ftdi_sio` das Gerät ab. Der
libftdi-Weg löst das, indem er den Kerneltreiber ablöst, plus eine udev-Regel.
D2XX gehört unter Linux nicht benutzt.

---

## 7. Das Bedienpult

*Settings → Devices*. Ein Behringer X-Touch über USB im **MC-Modus**.

**Der Port wird über seinen Namen ausgewählt**, und das ist die einzige
Möglichkeit, die ein Aus- und Einstecken übersteht: die Kennungen der
MIDI-Bibliothek sind Positionen in einer Liste, die sich umnummeriert. Die
Dekoration der Plattform wird vom Namen abgeschnitten, damit unter Linux nicht
die ALSA-Client-Nummer Teil der Konfiguration wird — die ändert sich über einen
Neustart hinweg.

**Ein Pult, das nicht eingesteckt ist, ist eine Warnung und kein
Startproblem**, und eines, das später eingesteckt wird, wird ohne Neustart
aufgenommen. Ein Pult, das nur **still** geworden ist, wird ausdrücklich *nicht*
neu geöffnet: das ist ein ausgeschaltetes Gerät und kein gezogenes Kabel.

`prismd --midi-ports` listet die Ports dieser Maschine und hört auf. Das ist der
schnellste Weg, den Namen zu bekommen, den Sie eintragen müssen.

Was welche Taste tut, steht in *Settings → Controls*, mit *Learn*. Die
Tastenbelegung liegt im Datenverzeichnis und geht mit, wenn das Pult umzieht.

> **Eine bekannte Eigenschaft der Hardware:** sättigt man beide Richtungen
> gleichzeitig, kann das X-Touch aufhören zu **senden**, während es weiter
> empfängt; nur Aus- und Einschalten holt es zurück. Dieses Pult ist dagegen
> designt — Rückmeldung geht als Differenz hinaus, nicht als Neuzeichnung —,
> aber wenn ein Haus das meldet, ist das die Erklärung.

---

## 8. Die Maschine: Netzwerkfreigabe, Token, Protokoll, Beenden

*Settings → This machine*.

### Der WebSocket-Zuhörer

Er ist **an**, auf `127.0.0.1:7373`. Das ist die Verbindung, über die das
eigene Fenster, ein zweiter Bildschirm auf derselben Maschine und die
Testsuite sprechen.

**Loopback ist das, was ihn ungefährlich macht.** Sobald Sie ihn auf eine
Adresse legen, die von anderen Maschinen erreichbar ist, **verlangt das Pult ein
Token** und weigert sich sonst. Das ist keine Bequemlichkeitsprüfung: ein
Lichtpult ohne Authentifizierung in einem Schulnetz ist genau die Sache, die
dort nicht hingehört.

**Ein Zuhörer, der nicht binden kann, ist eine Warnung und ein Pult, das
startet.** Zwei Daemons auf einer Maschine wollen beide 7373, und darüber den
Start zu verweigern hieße, dass ein verirrter Prozess ein Pult eine halbe Stunde
vor der Vorstellung unstartbar macht. Sie bekommen stattdessen eine Zeile: *hier
konfiguriert, hört nirgends zu*.

> **Das Token steht in `machine.json`.** Wenn Sie diese Datei an einen
> Fehlerbericht hängen, lesen Sie sie vorher.

### Universenzahl, Protokoll, Beenden

| Einstellung | Voreinstellung | Anmerkung |
|---|---|---|
| **Universen** | 64 | Wie viele Universen das Frame-Layout trägt, 1…64 |
| **Protokollstufe** | `info` | `debug`, `info`, `warn`, `error`, `off`. Vor einem reproduzierbaren Fehler hochdrehen |
| **Beim Beenden** | *hold* | *hold* lässt den letzten Look stehen, *blackout* schickt vorher einen Blackout auf die Bühne. In einem Haus mit Bewegungslicht ist das eine echte Entscheidung |

Manche dieser Einstellungen greifen erst nach einem Neustart des Daemons. Das
Fenster sagt bei jeder Zeile, welche das sind — es liest die Antwort aus dem
Modell und nicht aus einer zweiten Liste.

---

## 9. Autostart

Drei Stufen, und die mittlere ist die, die Sie in einer Schule wollen:

| Stufe | Windows | Rechte |
|---|---|---|
| **Voreinstellung** | Das Fenster startet `prismd` als Kind und lässt es beim Schließen laufen | keine |
| **Autostart** | Ein Wert unter `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | **keine** |
| **Dauerbetrieb** | Ein Windows-Dienst | Administrator |

Das Kästchen steht in *Settings → This machine*. Es startet das Pult beim
Anmelden in den Infobereich.

**Die Zeile darunter sagt, was die Maschine wirklich hat** — nicht, was
angehakt wurde. Der Eintrag wird bei **jedem** Start abgeglichen, was drei Fälle
auffängt: einen, den jemand von Hand gelöscht hat, eine Installation, die
umgezogen ist, und einen Schalter, der woanders umgelegt wurde. Löschen Sie den
Registrierungswert von Hand und öffnen Sie das Fenster wieder: die Zeile sagt,
dass er außerhalb des Programms entfernt wurde.

**Autostart gibt es nur unter Windows.** Die Einstellung existiert überall, der
Eintrag wird nur dort geschrieben. Der Grund ist ehrlich: Windows ist das
einzige Release-Ziel, kein CI-Job baut die Shell unter Linux oder macOS, und
ein Linux-Autostart wäre Code, den niemand baut.

---

## 10. Der Rechner ohne Bildschirm, und der Raspberry Pi

`prismd.exe` liegt neben `PrismDMX.exe` und läuft allein: eine Rack-Maschine
ohne Bildschirm, ein Raspberry Pi, eine feste Installation.

```
prismd --help          alle Schalter
prismd --midi-ports    die MIDI-Ports dieser Maschine, dann Ende
prismd                 mit der gespeicherten Konfiguration
```

**Ein Schalter gilt nur für diesen Lauf.** Nennen Sie Ausgänge auf der
Kommandozeile, sind diese das **ganze Rig für den Lauf**: `machine.json` wird
nicht gelesen und nicht überschrieben, und Kommandos, die die Ausgänge ändern
würden, werden abgelehnt. Das ist Absicht — ein Daemon, den jemand zum Probieren
mit `--mock-output` startet, darf weder die Verkabelung eines Hauses erben noch
sie überschreiben.

Ein Pult ohne Bildschirm bedient man über das X-Touch oder über einen zweiten
Rechner im selben Netz. Für den zweiten Rechner brauchen Sie den WebSocket auf
einer erreichbaren Adresse **und ein Token** (Kapitel 8).

### Raspberry Pi

Die Engine ist portabel und wird bei **jedem Commit** für ARM64
gegengeprüft, damit dieser Weg nicht unbemerkt verrottet. Was es heute *nicht*
gibt, ist ein fertiges Paket: es gibt keinen Release-Build für Linux und kein
Installationsprogramm.

Was ein Pi braucht:

1. **Ein Build aus dem Quelltext**, mit `--features prism-midi/alsa` und
   `libasound2-dev` installiert, wenn ein X-Touch daran soll. Ohne diese
   Funktion zählt das Pult null MIDI-Ports auf und öffnet keinen — dieselbe
   Antwort, die eine Maschine ohne angestecktes Gerät gibt.
2. **Den libftdi-Weg** für Open DMX USB, mit udev-Regel (Kapitel 6). Netzwerk
   ausgänge brauchen davon nichts.
3. **Autostart** über eine systemd-**Benutzer**-Unit; das Programm schreibt sie
   heute nicht selbst.

**Das ist noch niemand gegangen.** Der Pi-Weg ist als offener Punkt in
`ARCHITECTURE_SPEC.md` §14 und `PROGRESS.md` §5 verzeichnet, mit genau dem,
was daran zu messen ist: dass der Build durchgeht, dass die ALSA-Portnamen so
normalisiert werden, wie die Regel es vorhersagt, und dass die Client-Nummer
über einen Neustart hinweg wirklich wechselt. Wenn Sie ihn gehen, ist ein
Bericht darüber wertvoller als ein Fehlerbericht.

---

## 11. Abnahme: was Sie prüfen, bevor Sie gehen

Der Reihe nach. Jeder Punkt ist eine Sache, die ohne Prüfung erst in einer
Vorstellung auffällt.

**1. Ein Universe geht ins Leere.** *Settings → Outputs*: die Zeile, die die
gepatchten Universen nennt, die kein Ausgang trägt. Sie soll leer sein.

**2. Jeder Ausgang zählt Frames.** Dieselbe Tabelle, die Zählerspalte, eine
Sekunde hinsehen. Ein Ausgang, der nichts zählt, ist ein Ausgang, den niemand
bemerkt hat.

**3. Jede Art-Net-Node antwortet.** Unter der Zeile: *answering*, nicht
*never answered*. Steht dort *never answered*, ist es fast immer die Firewall
oder das Netzwerkprofil (Kapitel 5).

**4. Licht kommt an, wo es soll.** Ein Fixture, einmal durch, mit dem *DMX
Sheet* daneben offen. Das ist die Prüfung, die die Port-Adress-Zuordnung einer
Art-Net-Node erledigt, und ohne sie ist der Umstand *„die Node zählt ab null"*
etwas, das der Operator entdeckt.

**5. Das Fenster schließen tötet die Show nicht.** Fenster mit seinem eigenen
Knopf schließen. Das Symbol im Infobereich bleibt, `prismd.exe` steht noch im
Task-Manager, und ein *DMX Sheet* auf einem zweiten Client zeigt weiter
Bewegung. Dann das Programm noch einmal starten: das Fenster kommt zurück und es
gibt weiterhin **genau ein** `prismd.exe`.

**6. Der Dateidialog.** *Settings → Show files → Save as → Browse…* — der
Dialog des Betriebssystems geht auf, gefiltert auf `.prism`, in dem Ordner, in
dem die aktuelle Show liegt, und was gewählt wird, landet im Feld.

**7. Autostart.** Kästchen setzen, abmelden, anmelden: das Pult ist im
Infobereich. Der Registrierungswert steht unter
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Löschen Sie ihn von Hand
und öffnen Sie das Fenster wieder — die Zeile sagt, dass er außerhalb des
Programms entfernt wurde.

**8. Vollbild.** `F11`, dann `Alt` + `Enter`: das **Fenster** verliert die
Titelleiste und bekommt sie wieder, in beiden Richtungen.

**9. Ein abgeschossenes Pult wird bemerkt.** Bei laufendem Pult `prismd.exe`
im Task-Manager beenden. Innerhalb einer Sekunde liest der Tooltip im
Infobereich *the desk has stopped*, das Fenster kommt mit dem Satz nach vorn, und
das Symbol **verlässt** den Infobereich, wenn man es bestätigt. Danach das
Programm neu starten: genau ein Symbol.

**10. Sauber beenden.** Infobereich → *Stop the desk*. `prismd.exe` verschwindet,
und mit *blackout* als Beenden-Aktion geht das Rig dunkel, statt einzufrieren.

**11. Das Protokoll und die Prüfsumme aufschreiben.** Version aus *Settings →
This machine*, und wo `%APPDATA%\PrismDMX` liegt. Das ist das, wonach als Erstes
gefragt wird, wenn etwas gemeldet wird.

> Die Punkte 5 bis 10 sind dieselben, die `ARCHITECTURE_SPEC.md` §14 als
> „nur ein echter Desktop kann das beantworten" führt. Wenn Sie eine Abnahme
> gemacht haben, ist ein Bericht darüber der Beitrag, der dieser Beta am meisten
> hilft — er schließt Zeilen, die kein Test schließen kann.

---

## 12. Fehlersuche

| Symptom | Wo Sie zuerst hinsehen |
|---|---|
| Ausgang liest `Degraded` | *Settings → Outputs*, die Zeile darunter. Bei Art-Net: antwortet keine Node, oder hört das Pult nicht zu? Es steht dort, welches von beidem |
| Ausgang liest `Disconnected` | Bei Open DMX: Kabel und Seriennummer. Bei sACN: das Hop-Limit ließ sich nicht setzen |
| Art-Net-Nodes antworten nie | Firewall-Profil der Lichtnetzkarte (Kapitel 5). Das Pult schlägt es selbst vor |
| Licht auf falschen Kanälen | Port-Adress-Zuordnung der Node. Voreinstellung ist Universe *N* → *N − 1* |
| Licht an, das niemand programmiert hat | Ein zweiter Sender auf demselben Universe. Bei sACN entscheidet die Priorität |
| Das Programm startet, aber kein Fenster | Ein Pult läuft schon, und sein Zuhörer ist aus. Die Meldung sagt, welcher Prozess das Pult hält, wo er zu erreichen sein wollte und über welches Datenverzeichnis die beiden streiten |
| Das Fenster ist weiß | Die WebView2-Laufzeit fehlt (Kapitel 2) |
| Fixture-Bibliothek leer | `profiles\fixtures` neben dem Programm; eine Installation bringt sie mit. Aus dem Quelltext: `tools/fetch-fixtures/fetch-fixtures.ps1` |

**Das Protokoll** liegt in `%APPDATA%\PrismDMX`. Vor einem reproduzierbaren
Fehler die Stufe in *Settings → This machine* auf `debug` stellen.

**Melden** unter
[github.com/flakesystems/PrismDMX/issues](https://github.com/flakesystems/PrismDMX/issues).
Für einen Fehler an Ausgängen, Netz oder Bedienpult gehört `machine.json` dazu —
**vorher lesen**, sie enthält das Token dieses Pults, falls eines gesetzt ist.

---

## Weiter

| | |
|---|---|
| [Operator-Handbuch](operator.de.md) | Was ein Operator damit tut |
| [Entwicklerhandbuch](developer.en.md) | Wie das Programm gebaut ist |
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) §7 | Ausgänge, Feld für Feld |
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) §10.3 | Lebenszyklus und Autostart |
| [`../ISSUES.md`](../ISSUES.md) | Was bekannt nicht stimmt |
