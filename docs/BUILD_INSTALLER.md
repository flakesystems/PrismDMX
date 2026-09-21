# Den Installer bauen — `PrismDMX_<version>_x64-setup.exe`

Für den Test-Rig und für ein Release. **Nur unter Windows**: das Ergebnis ist
eine NSIS-`.exe`, und der Desktop-Aufsatz (`prism-app`) wird ohnehin nur dort
gebaut — `ci.yml` schließt ihn auf Linux aus.

Dasselbe tut `.github/workflows/release.yml` bei einem `v*`-Tag. Diese Datei ist
der Weg von Hand, für einen Build, der **nicht** veröffentlicht wird.

---

## 1. Was die Maschine braucht

| | |
|---|---|
| [Rust](https://rustup.rs) stable | 1.89 oder neuer, Ziel `x86_64-pc-windows-msvc` |
| MSVC Build Tools | „Desktop development with C++" |
| [Node](https://nodejs.org) | 24 |
| WebView2-Runtime | Windows 11 hat sie; Windows 10 ggf. nachinstallieren |

`7-Zip` braucht nur der CI-Schritt, der hineinschaut — für einen Build von Hand
nicht nötig.

---

## 2. Der Befehl

In einer PowerShell im Projektverzeichnis:

```powershell
# 1. Die TypeScript-Bindungen. Sie sind erzeugt und nicht eingecheckt,
#    und ohne sie typecheckt die Oberfläche nicht.
cargo test -p prism-domain

# 2. Die Abhängigkeiten der Oberfläche. Einmal pro Checkout.
cd ui
npm ci
cd ..

# 3. Die Engine als Release. Der Installer packt genau diese Datei ein.
cargo build --release -p prismd

# 4. Der Installer. Baut die Oberfläche selbst (beforeBuildCommand).
cd crates\prism-app
..\..\ui\node_modules\.bin\tauri build --config tauri.bundle.conf.json
cd ..\..
```

Das Ergebnis:

```
target\release\bundle\nsis\PrismDMX_0.9.2_x64-setup.exe
```

Die Versionsnummer im Dateinamen kommt aus `[workspace.package] version` in
`Cargo.toml` — heute **`0.9.2`**. Sie wird erst mit dem Release auf `0.9.3`
gezogen; ein Installer mit gleicher Nummer **überschreibt** eine installierte
Version, was für den Test-Rig genau richtig ist.

### Was installiert wird, und wohin

| Was | Wohin |
|---|---|
| `PrismDMX.exe` und `prismd.exe` daneben | `%LOCALAPPDATA%\PrismDMX` |
| `profiles\surface\xtouch.json` | daneben |
| `profiles\fixtures\` | daneben |
| Shows, `machine.json`, eigene Fixtures | `%APPDATA%\PrismDMX` — **rührt der Installer nicht an** |

Installiert wird **pro Benutzer** (`installMode: currentUser`), also ohne
Administratorrechte.

---

## 3. Die Falle: die Bibliothek wird mit eingepackt, wie sie gerade ist

`tauri.bundle.conf.json` packt `profiles\fixtures\` als Ressource ein. Der
Installer nimmt also **den Stand des Bauverzeichnisses**. Liegt dort nur
`SOURCE.md` — der Zustand eines frischen Checkouts —, dann bekommt das
installierte Pult **vier eingebaute Profile und sonst nichts**, und sagt das
beim Start auch.

Wer eine Bibliothek mit ausliefern will, installiert sie **vor Schritt 4**:

```powershell
# GDTF, aus einem Ordner mit .gdtf-Dateien …
$env:PRISMDMX_GDTF_SOURCE = "D:\gdtf"
tools\fetch-fixtures\fetch-fixtures.ps1

# … oder von gdtf-share.com, wofür es ein (kostenloses) Konto braucht:
$env:PRISMDMX_GDTF_USER = "…"
$env:PRISMDMX_GDTF_PASSWORD = "…"
tools\fetch-fixtures\fetch-fixtures.ps1

# Optional dazu: der Corpus der Open Fixture Library, nach profiles\fixtures\ofl\
tools\fetch-fixtures\fetch-ofl.ps1
```

**GDTF Share hat keinen anonymen Massen-Download** — das ist die Entscheidung
dieses Dienstes und nicht die dieses Projekts, und deshalb fragt das Skript nach
einem Ordner oder einem Konto, statt einfach zu laden. Ohne beides sagt es das
und bricht ab, ohne ein halb gefülltes Verzeichnis zu hinterlassen.

Zum Prüfen einzelner Fixtures braucht es das alles **nicht**: eine `.gdtf` im
eigenen Ordner (`%APPDATA%\PrismDMX\fixtures\`) wird gelesen und gewinnt, und
`docs/RELEASE_TEST_0.9.3.md` §1 baut sich eine Testdatei mit Bordmitteln.

---

## 4. Vorher alle Gates lokal fahren

`CLAUDE.md`, *CI Policy*: lokal vollständig, GitHub Actions minimal. Vor einem
Installer, der auf den Rig geht:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
$env:RUSTDOCFLAGS = "-D warnings"; cargo doc --workspace --no-deps
cargo test --workspace

cd ui
npx tsc -b --force
npm run lint
npm run test
npm run build
npm run e2e
cd ..
```

Die Rust- und die Browser-Suite **nacheinander**, nie überlappend: zwei
gleichzeitige `cargo test --workspace` scheitern mit `LNK1104` an einer
Testbinärdatei, die die andere offen hält.

Der Corpus-Test der Open Fixture Library **überspringt sich selbst**, wenn
`profiles\fixtures\ofl\` fehlt, und sagt das. Für GDTF gibt es keinen Corpus und
kann keinen geben; seine Tests bauen ihre Archive Byte für Byte und brauchen
nichts.

---

## 5. Ein echtes Release

Nicht von Hand. Ein Tag fährt `release.yml`, das alle Gates **auf Windows**
wiederholt, den Installer baut, hineinschaut, ob er trägt, was er soll, die
Prüfsumme rechnet und die GitHub-Release-Seite füllt:

```powershell
git tag v0.9.3
git push origin v0.9.3
```

Vorher muss `[workspace.package] version` in `Cargo.toml` **dieselbe** Nummer
tragen — der erste Schritt des Workflows vergleicht Tag und Manifest und bricht
sonst ab.

`release.yml` lässt sich auch ohne Tag über `workflow_dispatch` starten; dann
baut es und veröffentlicht nichts. Das ist der Weg, wenn eine Änderung die
Windows-Hälfte braucht, bevor ein Release ansteht — bewusst, nicht aus Gewohnheit.
