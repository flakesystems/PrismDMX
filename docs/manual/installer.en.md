# PrismDMX — Installer's manual

**For:** whoever sets the desk up in a building — outputs, network, machine,
autostart — not for whoever runs a show on it. That is the
[operator's manual](operator.en.md).
**Applies to:** version `0.9.2`.
**A note on language:** this manual exists in English and German. Anything the
program writes on screen, and anything that is a word on the command line, is
given in English because that is what it says.

---

## Contents

1. [What you are setting up](#1-what-you-are-setting-up)
2. [Installation](#2-installation)
3. [Where everything lives](#3-where-everything-lives)
4. [The outputs](#4-the-outputs)
5. [Network: sACN and Art-Net in one building](#5-network-sacn-and-art-net-in-one-building)
6. [Open DMX USB](#6-open-dmx-usb)
7. [The control surface](#7-the-control-surface)
8. [The machine: network exposure, token, log, shutdown](#8-the-machine-network-exposure-token-log-shutdown)
9. [Autostart](#9-autostart)
10. [The machine with no screen, and the Raspberry Pi](#10-the-machine-with-no-screen-and-the-raspberry-pi)
11. [Handover: what to check before you leave](#11-handover-what-to-check-before-you-leave)
12. [Fault-finding](#12-fault-finding)

---

## 1. What you are setting up

PrismDMX is **`prismd`** — the desk, which holds the show and puts out DMX — and
an **interface**, which only draws and sends. The desk process keeps running when
the window closes, and that is not a convenience: it is the reason for the split.

As the installer you set up three things, and they explicitly do **not** belong
in the show file:

| What | Where it lives | Why there |
|---|---|---|
| The outputs — which universes go where | `machine.json` | A rig is a property of the **building**. A show carried to the next hall on a stick must not bring the first one's cabling |
| This desk's identity (the sACN CID) | `machine.json` | For the same reason, and because a desk with a new CID is a **new source** to every receiver on every start |
| The surface's MIDI port, the log, the network exposure | `machine.json` | That is this machine, not this show |

**All of it is a setting in the window, not a command-line switch.** There is a
switch for each — `prismd --help` lists them — but a switch applies to **that run
only**: the stored setting is then neither read nor written, and the settings
window says which switch is currently holding which row. You set a building up in
the window.

---

## 2. Installation

**Windows 10 or 11, 64-bit.** `PrismDMX_<version>_x64-setup.exe` from the
[release page](https://github.com/flakesystems/PrismDMX/releases).

**It installs per user** and needs no administrator rights. That is deliberate: a
school laptop the operator is not allowed to administer is the normal case, and
everything up to and including autostart works without elevation. The price is
that the installation belongs to the account it ran under. If several accounts
are to operate the same desk, install once per account.

**Nothing has to be installed alongside it.** The whole build is linked against
the static C runtime; there is no Visual C++ redistributable to find. The one
exception is the **Edge WebView2 runtime**, which draws the window: Windows 11 has
it, Windows 10 has it anywhere Edge has been updated, and the installer fetches it
from Microsoft if it is missing. That is the only step that needs the internet —
in a building without it, install the WebView2 runtime by hand beforehand.

**SmartScreen.** The build is **not signed**, so on first start Windows shows
*Windows protected your PC*. If you would rather not click past that — a
reasonable position in somebody else's building — check the checksum instead:

```powershell
Get-FileHash .\PrismDMX_0.9.2_x64-setup.exe -Algorithm SHA256
```

and compare it with the one on the release page. That checksum is computed by the
same build run that produced the file; nobody types it out.

**Updating** means running the new installer. It replaces the program in place,
does not touch your data and does not stop a running desk — though a running desk
stays the old build until it is restarted.

---

## 3. Where everything lives

| | Where (Windows) |
|---|---|
| The program, `PrismDMX.exe` with `prismd.exe` beside it | `%LOCALAPPDATA%\PrismDMX` |
| Shows, `machine.json`, your own fixture profiles, key bindings, the log | `%APPDATA%\PrismDMX` |
| The bundled fixture library | beside the program, `profiles\fixtures` |

`%APPDATA%\PrismDMX` is the **data directory**. It is the one setting that is read
and never written, and the reason is that the settings live *inside* it: a desk
told to move house would have to learn that somewhere else. If you must relocate
it, the way is `--data-dir` or the environment variable `PRISMD_DATA_DIR`, and the
settings window names the directory so `machine.json` can be found.

**Uninstalling removes the program and nothing else.** The data directory stays,
and a reinstallation finds it again. There is no version of *I want to reinstall
this* that also means *throw my show away*.

Two files in the data directory are operating state rather than configuration:

- **`prismd.guard`** — an empty file on which the running daemon holds an
  operating-system lock. It is the reason there are never two desks on one rig:
  the question is *is anybody holding this*, not *is process 4711 alive*, and the
  operating system releases the lock even when the process was killed. A file left
  behind blocks nothing.
- **`prismd.lock`** — beside it, readable, with the process id, the endpoints and
  the token. It is how a window finds its desk.

---

## 4. The outputs

*Settings → Outputs*. An output is a row with five things on it:

| | |
|---|---|
| **Number and name** | The name is for people. Changing it costs **nothing** — no restart, no frame |
| **Kind** | Art-Net, sACN or Open DMX USB |
| **Universes** | Which universes *this* output carries. Not all of them |
| **Enabled** | A disabled output keeps its whole configuration and has no thread and no socket — which is what you want of a node somebody is working on |

**Both directions are allowed:** a universe may go to several outputs, and an
output may carry many. A universe going to two outputs arrives at both **byte for
byte identical**.

**A patched universe that no output carries is a legal state, and it is
reported.** Forbidding the desk to patch into a universe that is not wired yet
would be a desk you cannot prepare before the install; dropping it silently is how
a universe stays dark while every lamp is green. *Settings → Outputs* names those
universes. **Read that line at handover.**

**What costs an output a restart:** the kind, the universes, the number, the
enabled switch. Nothing else. Outputs that did not change cost a rebuild **no
frame and no tick** — you may add a node while the show runs.

**The frame counter** in the row is asked once a second while the window is open.
It is the way to see whether a row you just created is really doing anything.

---

## 5. Network: sACN and Art-Net in one building

### The general rule

**This desk never broadcasts**, unless you ask for it by name. Art-Net goes
**unicast** to the addresses you enter — an Art-Net broadcast floods a school
network — and sACN goes multicast to the universe's group address, or unicast to a
named receiver.

**Both send on change and otherwise as a refresh.** Sending 64 universes at 44 Hz
unchanged would be 1.5 MB/s of nothing.

### Art-Net

| Setting | What you need to know |
|---|---|
| **Destination** | Host and port. The port is 6454 if you name none |
| **Port address** | Universe → Net · Sub-Net · Universe **on that node**. Default: universe *N* → port address *N − 1*, because PrismDMX counts universes from 1 and Art-Net from 0. A four-way node is four rows you read off the back of the device — **nodes do not agree about this**, which is why it is configurable |
| **ArtSync** | Off. A node that understands it shows nothing further until one arrives. If you turn it on, it goes to **the same** addresses as the data — a unicast configuration is not made a broadcasting one by it |
| **Refresh** | At least every 800 ms, and one cadence *early*, so the gap does not become 800 ms plus a cadence |

**Node discovery.** The desk sends an `ArtPoll` every three seconds to **exactly
the addresses it already sends to** — nowhere else, and never as a broadcast. What
comes back is in *Settings → Outputs* under the row: the node's name, its IP, and
whether it is *answering*, has *never answered* or has *stopped*, with the age of
the last reply beside it.

That is the difference between *the socket took the datagram* and *somebody
received it*. An Art-Net output whose nodes are silent reads **Degraded**, not
*OK*.

**A node whose address nobody entered** is only found if it **announces itself** —
which nodes do on power-up and on a configuration change, and the desk listens on
Art-Net's own port.

> **The firewall is the case that has already bitten in a real building.** The
> inbound rule covered the *Private* profile, the lighting network was *Public*,
> and every reply was discarded before the process saw it. The desk now says so
> itself: if polls go out and nothing at all comes back, *Settings → Outputs*
> carries a suggestion of what to check. In that case, check which network profile
> Windows has assigned to the lighting network adapter.

### sACN (E1.31)

| Setting | What you need to know |
|---|---|
| **CID** | This desk's identity, a UUID, **configuration**. An output with no CID does not connect, rather than sending under the null CID every unconfigured desk would share |
| **Addressing** | Multicast `239.255.<universe high>.<universe low>`, port 5568. Unicast to named receivers for buildings that forbid multicast |
| **Universe numbers** | Universe *N* → E1.31 universe *N*. Both count from 1 — configurable anyway, because a building's universe 1 is not always a desk's universe 1 |
| **Priority** | **Per universe**, 0…200, default 100. This is how a backup desk takes over: two sources on one universe are decided by the higher priority. Above 200 is refused, not clamped |
| **Source name** | 64 bytes from the show file, truncated at a character boundary |
| **Hop limit (TTL)** | Set explicitly on connect, default 1. A hop limit that could not be set makes the output **Disconnected** — otherwise you would have ordered a routed lighting network and got datagrams that stop at the first router, under a green lamp |
| **Shutting down** | Three `Stream_Terminated` packets per active universe, each with the **next** sequence number |

**What a switch has to be able to do:** IGMP snooping, or every universe goes to
every port. If the building is routed, the hop limit has to go up — it is a
setting, not a code problem.

**Two senders on one universe** is the commonest fault in a building with two
desks, and neither of them notices on its own. Priority is the remedy, and it
belongs written down at handover.

---

## 6. Open DMX USB

The supported adapter is an **FTDI FT232R with no microcontroller** — verified
with a DSD TECH SH-RS09B (`0403:6001`, product string `FT232R USB UART`). There is
no widget firmware producing break and mark-after-break, so the **computer** does
the DMX timing.

Three things you have to tell a building:

1. **Exactly one universe per adapter.** Two is refused, not truncated.
2. **About 35 Hz**, not 44. Measured: 35.5 Hz over 60 s. That is normal for this
   design — QLC+ and FreeStyler get no more out of the same cable — and it is
   unproblematic for dimmers and LED PARs. Anybody who needs a guaranteed 44 Hz
   takes Art-Net or sACN. The desk says so itself when the output is created.
3. **Two adapters are told apart by serial number.** It is in the output row.
   Without it, every output takes the first adapter it finds.

**A pulled cable is the expected case**, not the exception: the thread notices the
fault, cleans up and reconnects, the lamp goes red and the engine keeps running.

**On Linux and on the Raspberry Pi** `ftdi_sio` grabs the device. The libftdi
route solves it by detaching the kernel driver, plus a udev rule. D2XX is not to
be used on Linux.

---

## 7. The control surface

*Settings → Devices*. A Behringer X-Touch over USB in **MC mode**.

**The port is selected by its name**, and that is the only way that survives
unplugging and replugging: the MIDI library's identifiers are positions in a list
that renumbers itself. Platform decoration is trimmed off the name so that on
Linux the ALSA client number does not become part of the configuration — it
changes across a reboot.

**A surface that is not plugged in is a warning and not a start-up problem**, and
one plugged in later is picked up without a restart. A surface that has merely
gone **quiet** is explicitly *not* reopened: that is a device switched off, not a
cable pulled.

`prismd --midi-ports` lists this machine's ports and stops. It is the quickest way
to get the name you have to enter.

What each key does is in *Settings → Controls*, with *Learn*. The key bindings
live in the data directory and travel with the desk.

> **A known property of the hardware:** saturate both directions at once and the
> X-Touch can stop **sending** while it goes on receiving; only a power cycle
> brings it back. This desk is designed against it — feedback goes out as a
> difference rather than a redraw — but if a building reports it, that is the
> explanation.

---

## 8. The machine: network exposure, token, log, shutdown

*Settings → This machine*.

### The WebSocket listener

It is **on**, at `127.0.0.1:7373`. That is the connection the desk's own window, a
second screen on the same machine, and the test suite all speak over.

**Loopback is what makes it harmless.** The moment you put it on an address other
machines can reach, **the desk requires a token** and refuses otherwise. That is
not a convenience check: a lighting desk with no authentication on a school
network is exactly the thing that does not belong there.

**A listener that cannot bind is a warning and a desk that starts.** Two daemons
on one machine both want 7373, and refusing to start over that would mean a stray
process can make a desk unstartable half an hour before a performance. You get a
line instead: *configured here, listening nowhere*.

> **The token is in `machine.json`.** If you attach that file to a fault report,
> read it first.

### Universe count, log, shutdown

| Setting | Default | Note |
|---|---|---|
| **Universes** | 64 | How many universes the frame layout carries, 1…64 |
| **Log level** | `info` | `debug`, `info`, `warn`, `error`, `off`. Turn it up before a reproducible fault |
| **On shutdown** | *hold* | *hold* leaves the last look standing, *blackout* sends a blackout to the stage first. In a building with moving lights that is a real decision |

Some of these settings only take effect after the daemon restarts. The window says
which, on every row — it reads the answer out of the model rather than out of a
second list.

---

## 9. Autostart

Three levels, and the middle one is what you want in a school:

| Level | Windows | Rights |
|---|---|---|
| **Default** | The window starts `prismd` as a child and leaves it running when it closes | none |
| **Autostart** | A value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | **none** |
| **Permanent** | A Windows service | administrator |

The checkbox is in *Settings → This machine*. It starts the desk into the
notification area at logon.

**The line under it says what the machine actually has** — not what was ticked.
The entry is reconciled on **every** start, which catches three cases: one somebody
deleted by hand, an installation that moved, and a switch thrown somewhere else.
Delete the registry value by hand and open the window again: the line says it was
removed outside the program.

**Autostart exists only on Windows.** The setting exists everywhere, the entry is
only written there. The reason is honest: Windows is the only release target, no
CI job builds the shell on Linux or macOS, and a Linux autostart would be code
nobody builds.

---

## 10. The machine with no screen, and the Raspberry Pi

`prismd.exe` sits beside `PrismDMX.exe` and runs on its own: a rack machine with
no screen, a Raspberry Pi, a fixed installation.

```
prismd --help          every switch
prismd --midi-ports    this machine's MIDI ports, then stop
prismd                 with the stored configuration
```

**A switch applies to that run only.** Name outputs on the command line and those
are the **whole rig for the run**: `machine.json` is neither read nor overwritten,
and commands that would change the outputs are refused. That is deliberate — a
daemon somebody starts with `--mock-output` to try something must neither inherit
a building's cabling nor overwrite it.

A desk with no screen is operated from the X-Touch or from a second machine on the
same network. For the second machine you need the WebSocket on a reachable address
**and a token** (chapter 8).

### Raspberry Pi

The engine is portable and is cross-checked for ARM64 on **every commit**, so this
route cannot rot unnoticed. What does *not* exist today is a finished package:
there is no release build for Linux and no installer.

What a Pi needs:

1. **A build from source**, with `--features prism-midi/alsa` and `libasound2-dev`
   installed if an X-Touch is going on it. Without that feature the desk
   enumerates zero MIDI ports and opens none — the same answer a machine with
   nothing plugged in gives.
2. **The libftdi route** for Open DMX USB, with a udev rule (chapter 6). Network
   outputs need none of it.
3. **Autostart** via a systemd **user** unit; the program does not write one
   itself today.

**Nobody has walked this yet.** The Pi route is recorded as an open item in
`ARCHITECTURE_SPEC.md` §14 and `PROGRESS.md` §5, with exactly what is to be
measured about it: that the build goes through, that the ALSA port names normalise
the way the rule predicts, and that the client number really does change across a
reboot. If you walk it, a report about that is worth more than a fault report.

---

## 11. Handover: what to check before you leave

In order. Every item is something that otherwise first shows up during a
performance.

**1. A universe goes nowhere.** *Settings → Outputs*: the row naming the patched
universes no output carries. It should be empty.

**2. Every output counts frames.** Same table, the counter column, watch for a
second. An output counting nothing is an output nobody noticed.

**3. Every Art-Net node answers.** Under the row: *answering*, not *never
answered*. If it says *never answered*, it is almost always the firewall or the
network profile (chapter 5).

**4. Light arrives where it should.** One fixture, run through, with the *DMX
Sheet* open beside it. This is the check that settles an Art-Net node's port
address mapping, and without it *"the node counts from zero"* is something the
operator discovers.

**5. Closing the window does not kill the show.** Close the window with its own
button. The notification-area icon stays, `prismd.exe` is still in Task Manager,
and a *DMX Sheet* on a second client still shows movement. Then start the program
again: the window comes back and there is still **exactly one** `prismd.exe`.

**6. The file dialog.** *Settings → Show files → Save as → Browse…* — the
operating system's dialog opens, filtered to `.prism`, in the folder the current
show is in, and what is chosen lands in the field.

**7. Autostart.** Tick the box, log out, log in: the desk is in the notification
area. The registry value is under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Delete it by hand and open
the window again — the line says it was removed outside the program.

**8. Full screen.** `F11`, then `Alt` + `Enter`: the **window** loses its title bar
and gets it back, both ways.

**9. A killed desk is noticed.** With the desk running, end `prismd.exe` in Task
Manager. Within a second the notification-area tooltip reads *the desk has
stopped*, the window comes forward with the sentence, and the icon **leaves** the
notification area when it is acknowledged. Then restart the program: exactly one
icon.

**10. Shutting down cleanly.** Notification area → *Stop the desk*. `prismd.exe`
disappears, and with *blackout* as the shutdown action the rig goes dark instead
of freezing.

**11. Write down the log and the checksum.** The version from *Settings → This
machine*, and where `%APPDATA%\PrismDMX` is. That is the first thing asked for
when something is reported.

> Items 5 to 10 are the same ones `ARCHITECTURE_SPEC.md` §14 lists as *only a real
> desktop can answer this*. If you have done a handover, a report about it is the
> contribution that helps this beta most — it closes rows no test can close.

---

## 12. Fault-finding

| Symptom | Where to look first |
|---|---|
| Output reads `Degraded` | *Settings → Outputs*, the row beneath. For Art-Net: is no node answering, or is the desk not listening? It says which |
| Output reads `Disconnected` | For Open DMX: cable and serial number. For sACN: the hop limit could not be set |
| Art-Net nodes never answer | The lighting adapter's firewall profile (chapter 5). The desk suggests it itself |
| Light on the wrong channels | The node's port address mapping. The default is universe *N* → *N − 1* |
| Light on that nobody programmed | A second sender on the same universe. With sACN, priority decides |
| The program starts but no window | A desk is already running and its listener is off. The message says which process holds the desk, where it wanted to be reachable, and which data directory the two are arguing about |
| The window is white | The WebView2 runtime is missing (chapter 2) |
| Fixture library empty | `profiles\fixtures` beside the program; an installation brings it. From source: `tools/fetch-fixtures/fetch-fixtures.ps1` |

**The log** is in `%APPDATA%\PrismDMX`. Before a reproducible fault, set the level
in *Settings → This machine* to `debug`.

**Report** at
[github.com/flakesystems/PrismDMX/issues](https://github.com/flakesystems/PrismDMX/issues).
For a fault in the outputs, the network or the control surface, `machine.json`
belongs with it — **read it first**, it contains this desk's token if one is set.

---

## Where to go next

| | |
|---|---|
| [Operator's manual](operator.en.md) | What an operator does with it |
| [Developer's manual](developer.en.md) | How the program is built |
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) §7 | Outputs, field by field |
| [`../../ARCHITECTURE_SPEC.md`](../../ARCHITECTURE_SPEC.md) §10.3 | Lifecycle and autostart |
| [`../ISSUES.md`](../ISSUES.md) | What is known not to work |
