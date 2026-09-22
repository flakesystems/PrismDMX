# PrismDMX — signing and notarising the macOS build

**For:** whoever holds, or is about to buy, the Apple Developer account this
project signs with. It is a setup document, not something an operator or an
installer ever reads — they get a `.dmg` that opens, which is the whole point of
it.
**Applies to:** version `0.9.2` and later, on macOS 11 and later.
**Added by:** S63, the session that made the desk build on a Mac.

> **The short version.** You need a **Developer ID Application** certificate and
> an **app-specific password**, both from one Apple Developer Program account.
> The certificate signs the bundle; the password notarises it. **Six** values go
> into GitHub Secrets and `.github/workflows/release.yml` does the rest. §5, §6
> and §9 are the three sections you cannot skip — and **§5.0 does the
> certificate on the command line**, which is shorter than the windows and does
> not depend on what language your macOS is in.

---

## Contents

1. [What signing is for, and the three states a build can be in](#1-what-signing-is-for-and-the-three-states-a-build-can-be-in)
2. [What you need before you start](#2-what-you-need-before-you-start)
3. [Which certificate — and why it is not the one Xcode offers you](#3-which-certificate--and-why-it-is-not-the-one-xcode-offers-you)
4. [Provisioning profiles: why this project has none](#4-provisioning-profiles-why-this-project-has-none)
5. [Making the certificate](#5-making-the-certificate)
6. [The notarisation credentials](#6-the-notarisation-credentials)
7. [How PrismDMX is configured to be signed](#7-how-prismdmx-is-configured-to-be-signed)
8. [Signing on your own Mac](#8-signing-on-your-own-mac)
9. [GitHub Actions: the six secrets and the job that reads them](#9-github-actions-the-six-secrets-and-the-job-that-reads-them)
10. [Checking the result — the four commands that actually prove it](#10-checking-the-result--the-four-commands-that-actually-prove-it)
11. [When it goes wrong](#11-when-it-goes-wrong)
12. [Expiry, revocation and handover](#12-expiry-revocation-and-handover)

---

## 1. What signing is for, and the three states a build can be in

macOS refuses to open software it cannot attribute to somebody. There are three
states a `PrismDMX.dmg` can be in, and the difference between them is what a
person in a school actually experiences:

| State | What the operator sees | What it costs you |
|---|---|---|
| **Unsigned** | *"PrismDMX" is damaged and can't be opened. You should move it to the Bin.* — which is a lie, but it is what Gatekeeper says about an unsigned quarantined download. There is no *Open anyway* on modern macOS for a `.dmg` from the internet. | nothing |
| **Signed, not notarised** | *"PrismDMX" can't be opened because Apple cannot check it for malicious software.* Right-click → **Open** works, once, per machine. | the certificate (§5) |
| **Signed and notarised** | It opens. No dialog, no right-click, no explanation to anybody. | the certificate **and** the app-specific password (§6) |

Aim for the third. The second is not a resting place — it is what you get when
half the secrets are configured, and §9's job warns loudly rather than letting
you ship it by accident.

**None of this is about trust in the code.** A Developer ID signature says *this
came from this Apple account and has not been altered since*. It does not say
the software is safe, and notarisation is an automated malware scan rather than
a review. What it buys is that the file opens.

---

## 2. What you need before you start

| | |
|---|---|
| **An Apple Developer Program membership** | **99 USD / 99 EUR a year.** A free Apple ID is not enough: it can produce an *Apple Development* certificate, which does not distribute (§3). Enrol at [developer.apple.com/programs](https://developer.apple.com/programs/). |
| **The right role in that account** | **Account Holder** or **Admin.** A *Developer* role can create certificates but **cannot** create a Developer ID one — that is restricted, deliberately, because a Developer ID certificate signs software in the organisation's name. |
| **A Mac** | Certificates are made with **Keychain Access**, which is macOS only. |
| **Xcode** | Full Xcode from the App Store, not just the Command Line Tools: `xcrun notarytool` ships with Xcode. Signing itself only needs the Command Line Tools, so a machine with just those can build and sign but not notarise. |
| **Roughly an hour** | Most of it is Apple's web interface. The certificate itself takes two minutes. |

**An organisation account and an individual one differ in one way that matters.**
On an *organisation* account, the Developer ID certificate belongs to the
organisation and only the Account Holder may create it, and there is a hard limit
on how many can exist (**five** of each Developer ID type, ever, per account —
with revocation needed to free a slot). On an *individual* account it is yours.
Either way, treat the private key as the most valuable file in the project: it
is not recoverable, and §12 is what to do about that before you need it.

---

## 3. Which certificate — and why it is not the one Xcode offers you

Apple issues several kinds and their names are close enough to be genuinely
confusing. This is the whole table, and the two rows that matter to PrismDMX are
marked:

| Certificate | Signs | Distributed how | PrismDMX |
|---|---|---|---|
| **Developer ID Application** | `.app`, and anything inside it | Downloaded from anywhere — your own site, GitHub Releases | **← this one** |
| **Developer ID Installer** | `.pkg` installers | the same | only if the project ever ships a `.pkg` (§7) |
| Apple Development | `.app`, for running on your own registered machines | not at all | no |
| Apple Distribution | `.app`, for the App Store | App Store Connect | no |
| Mac App Distribution / Mac Installer Distribution | `.app` / `.pkg` for the Mac App Store | Mac App Store | no |

**PrismDMX needs `Developer ID Application`, and usually nothing else.** It
ships a `.dmg`, and a `.dmg` is not signed as an installer — the *application
inside it* is signed with the Application certificate, and the disk image itself
carries a signature made with that same certificate. The **Installer**
certificate is only for `.pkg` files, which this project does not build.

> **If your Mac already shows `Apple Development: Your Name (XXXXXXXXXX)` in
> `security find-identity -v -p codesigning`, that is not the certificate you
> want.** It is what Xcode creates on its own the first time you build, it signs
> for your own machines only, and a bundle signed with it is refused on every
> other Mac in the world. §5 is still ahead of you.

---

## 4. Provisioning profiles: why this project has none

A provisioning profile ties a signing certificate to a list of **capabilities**
and, for development builds, to a list of **registered devices**. You have
probably read that you need one. For PrismDMX you do not, and it is worth
knowing exactly why so that you recognise the day it changes.

**Developer ID distribution requires no provisioning profile.** The profile
exists to authorise entitlements the operating system will not grant on a
signature alone — iCloud, Push Notifications, App Groups, Apple Pay, the App
Sandbox with certain exceptions. PrismDMX uses **none** of them:
`crates/prism-app/entitlements.plist` asks for exactly one key,
`com.apple.security.cs.allow-jit`, which is a hardened-runtime exception and not
a provisioned capability. So there is nothing for a profile to authorise.

**When you would need one.** If PrismDMX ever gains a capability from that list
the build changes in three places at once, and this is the order:

1. **Enable the capability** on the App ID at
   *Certificates, Identifiers & Profiles* → **Identifiers** → the
   `de.prismdmx.desk` App ID. (There is no App ID today — Developer ID builds do
   not need one either.)
2. **Create a Developer ID provisioning profile** for that App ID under
   **Profiles** → **+** → *Distribution* → **Developer ID**, download the
   `.provisionprofile`, and copy it to
   `crates/prism-app/embedded.provisionprofile`.
3. **Point Tauri at it**: add `"provisionProfile"` alongside `"entitlements"` in
   `crates/prism-app/tauri.bundle.macos.conf.json`, and add the profile to the
   CI job as a sixth secret, written to that path before the build.

Until one of those capabilities is actually wanted, a profile in the bundle is
one more thing that expires — profiles last a year and a **build made with an
expired one fails to launch**, which is a worse failure than not having had one.

---

## 5. Making the certificate

Four steps, in this order. Steps 1 and 4 are on your Mac; 2 and 3 are on Apple's
website.

### 5.0 The short way, on the command line — **and it is language-independent**

**Added after the owner pointed out that §5.1's menu names are the English
ones.** macOS is localised and this document is not: on a German system the
menus below read *Schlüsselbundverwaltung → Zertifikatsassistent → „Zertifikat
einer Zertifizierungsinstanz anfordern …"*, and the field labels differ again.

The whole of §5.1 and §5.4 can be done with `openssl` instead, which says the
same thing in every language, can be pasted, and produces the `.p12` §9 wants
directly rather than through an export dialogue. **This is the recommended
path**; §5.1 stays for whoever prefers the window.

**Use `/usr/bin/openssl` explicitly** — see the warning below.

1. A private key and a certificate signing request:

```bash
/usr/bin/openssl genrsa -out prismdmx.key 2048
```

```bash
/usr/bin/openssl req -new -key prismdmx.key -out prismdmx.certSigningRequest -subj "/CN=PrismDMX Developer ID"
```

> **The subject does not matter, and it is worth knowing that before you worry
> about it.** Apple takes the *public key* out of the request and issues a
> certificate whose name it composes itself — `Developer ID Application: <your
> team> (<TEAMID>)` — so the `CN` above is a label for your own request file
> and nothing else, and an e-mail address in it is not used at all. Add
> `/emailAddress=…` if you like the record; leave it out and nothing changes.
>
> **The address that *must* be right is `APPLE_ID` (§6 and §9)**: the Apple ID
> of the Developer Program account, the same one the app-specific password
> belongs to. A wrong one there fails notarisation with *Invalid credentials*.

2. Upload `prismdmx.certSigningRequest` at §5.2 and download
   `developerID_application.cer`.

3. Apple returns **DER**. Convert it and pack both halves into the `.p12`:

```bash
/usr/bin/openssl x509 -inform DER -in developerID_application.cer -out cert.pem
```

```bash
/usr/bin/openssl pkcs12 -export -out Certificates.p12 -inkey prismdmx.key -in cert.pem
```

It asks for an export password. That is `APPLE_CERTIFICATE_PASSWORD` (§9) —
long, random, into your password manager now.

4. For signing **locally** as well, put the same file in your keychain:

```bash
security import Certificates.p12 -k ~/Library/Keychains/login.keychain-db -T /usr/bin/codesign
```

5. Check it arrived, which is §5.3's test either way:

```bash
security find-identity -v -p codesigning | grep "Developer ID Application"
```

> ### ⚠️ Which `openssl` — this one bites, and its error message lies
>
> macOS ships **LibreSSL** at `/usr/bin/openssl`. Homebrew installs **OpenSSL
> 3** and usually puts it *earlier* in `PATH`, so a bare `openssl` is often not
> the one you think. They do not produce the same `.p12`:
>
> | Built with | `security import` says |
> |---|---|
> | `/usr/bin/openssl` (LibreSSL) | **imports** |
> | Homebrew OpenSSL 3, no flag | **`MAC verification failed during PKCS12 import (wrong password?)`** |
> | Homebrew OpenSSL 3, `-legacy` | **imports** |
>
> OpenSSL 3 defaults to an encryption macOS's importer does not read, and the
> error it produces **blames the password**, which is the one thing that is not
> wrong. All three rows were measured on a real keychain import.
>
> So: use `/usr/bin/openssl`, or add `-legacy` to the `pkcs12 -export` line.
> `openssl version` tells you which you have — LibreSSL, or OpenSSL 3.

---

### 5.1 Make the signing request on your Mac

The private key is created here and **never leaves this machine**. What you send
Apple is a request containing only the public half.

**The menu names below are the English ones.** On a German system they read
*Schlüsselbundverwaltung*, *Zertifikatsassistent* and *„Zertifikat einer
Zertifizierungsinstanz anfordern …"*; the German names in this section were
read out of the application's own localisation tables, the field labels inside
the dialogue were not and may be worded differently. **§5.0 avoids all of it.**

1. Open **Keychain Access** — German: **Schlüsselbundverwaltung**
   (`/Applications/Utilities/`, German *Dienstprogramme*).
2. Menu **Keychain Access → Certificate Assistant → Request a Certificate From a
   Certificate Authority…** — German: **Schlüsselbundverwaltung →
   Zertifikatsassistent → „Zertifikat einer Zertifizierungsinstanz anfordern …"**
3. Fill it in:
   - **User Email Address**: the Apple ID of the developer account. (Like the
     `CN` in §5.0, this is not carried into the issued certificate — Apple
     composes that name itself. It is a label on your own request.)
   - **Common Name**: something you will recognise in a keychain list, e.g.
     `PrismDMX Developer ID`.
   - **CA Email Address**: leave **empty**.
   - Select **Saved to disk**, and tick **Let me specify key pair information**.
4. **Continue**, choose where to save the `.certSigningRequest`.
5. Key pair information: **Key Size 2048 bits**, **Algorithm RSA**. These are not
   preferences — Apple rejects anything else.
6. **Continue** → **Done**.

You now have two things: a `CertificateSigningRequest.certSigningRequest` file,
and — invisibly — a new private key in your **login** keychain. Keychain Access
→ **login** → **Keys** will show it under the Common Name you gave.

> **Do not delete that key.** Without it the certificate Apple issues in a
> moment is a public document that can sign nothing.

### 5.2 Ask Apple for the certificate

1. Sign in at
   [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates/list).
2. **+** (Create a certificate).
3. Under **Software**, choose **Developer ID Application**.
   - If that option is **missing or greyed out**, you are not the Account Holder
     or an Admin (§2), or you are signed into the wrong team. Use the team
     selector at the top right.
4. If asked for a **profile type**, choose **Direct** (as opposed to *G2 Sub-CA*)
   unless you specifically need the newer intermediate; either works, and Apple's
   own default is correct.
5. **Choose File** → the `.certSigningRequest` from §5.1 → **Continue**.
6. **Download** the resulting `developerID_application.cer`.

### 5.3 Install it into the keychain

**Double-click the downloaded `.cer` file.** Keychain Access opens and files it
in your **login** keychain.

Then check that it married up with the private key from §5.1 — this is the step
people skip and it is the one that fails later:

```bash
security find-identity -v -p codesigning
```

You are looking for a line like:

```
  1) A1B2C3D4E5F6A7B8C9D0E1F2A3B4C5D6E7F8A9B0 "Developer ID Application: Your Name (ABCDE12345)"
     1 valid identities found
```

**If it does not appear**, or appears under *Certificates* in Keychain Access
**without a disclosure triangle** next to it, the certificate is not paired with
its private key. The key is missing or is in a different keychain, and the
certificate has to be re-issued against a new request (§5.1). Apple cannot send
you the key; nobody has it but you.

The ten characters in the brackets — `ABCDE12345` above — are your **Team ID**,
*on a Developer ID certificate*. Write it down; §6 and §9 both want it.

> **The brackets are not always the Team ID, and getting this wrong costs an
> afternoon.** On an **Apple Development** certificate the brackets hold the
> *individual's* identifier, which is a different string from the team's — this
> machine's Development identity reads
> `Apple Development: … (FPH8XF8FP4)` while its `TeamIdentifier` is
> `RS8R9CG44U`. Notarisation given the wrong one fails with *Invalid
> credentials*, which reads like a bad password and is not. If you have both
> kinds of certificate installed, read the Team ID off the **signature** rather
> than the name:
>
> ```bash
> codesign --display --verbose=2 /path/to/Something.app 2>&1 | grep TeamIdentifier
> ```
>
> or from *Membership details* at
> [developer.apple.com/account](https://developer.apple.com/account), which is
> unambiguous.

### 5.4 Export the `.p12`, for CI and for a backup

CI has no keychain, so it is handed the certificate and its private key together
in one encrypted file.

1. Keychain Access → **login** → **My Certificates**. German: **Anmeldung** →
   **Meine Zertifikate** (the private key sits under **Schlüssel**).
2. Find **Developer ID Application: …**, and **expand the disclosure triangle**
   so you can see the private key underneath it.
3. Right-click the **certificate** row (not the key) → **Export "Developer ID
   Application: …"**.
4. File format **Personal Information Exchange (.p12)**. Save it.
5. It asks for a password to protect the file. **Use a long random one and put it
   in your password manager now** — it becomes the `APPLE_CERTIFICATE_PASSWORD`
   secret in §9 and there is no way to recover it.
6. It then asks for your **login keychain** password, to let the key out. That is
   your Mac account password, and it is not the same thing as step 5's.

You now have `Certificates.p12`. **It is equivalent to the ability to sign
software in your name.** Keep it out of the repository — `.gitignore` does not
know about it and will not save you — and put a copy somewhere a house fire
cannot reach (§12).

---

## 6. The notarisation credentials

Signing proves who built it. **Notarisation** is Apple scanning the result and
issuing a ticket that says it did. It needs three values, none of which is the
certificate.

### 6.1 The app-specific password

Your real Apple ID password is not used, and with two-factor authentication on
the account it would not work anyway.

1. Go to [account.apple.com](https://account.apple.com) and sign in.
2. **Sign-In and Security** → **App-Specific Passwords**.
3. **+**, name it something you will recognise later — `PrismDMX notarisation` —
   and **Create**.
4. Copy the password. It looks like `abcd-efgh-ijkl-mnop`. **It is shown once.**

### 6.2 The Team ID

The ten characters from §5.3 — **from the Developer ID certificate**, and see
the warning there if you also have an Apple Development one installed. It is
also at the top right of
[developer.apple.com/account](https://developer.apple.com/account) under
*Membership details*, which is the reading that cannot be ambiguous, and from a
terminal:

```bash
security find-identity -v -p codesigning | grep "Developer ID Application" | sed -n 's/.*(\([A-Z0-9]\{10\}\)).*/\1/p'
```

The `grep` is the part that matters: without it the command answers with
whichever identity happens to be listed first, which on a developer's own
machine is usually the Apple Development one.

### 6.3 Check them before you need them

Do this now rather than inside a failing release. It stores the three values in
your keychain under a name of your choosing and then tries them:

```bash
xcrun notarytool store-credentials "prismdmx" --apple-id "you@example.com" --team-id "ABCDE12345" --password "abcd-efgh-ijkl-mnop"
```

```bash
xcrun notarytool history --keychain-profile "prismdmx"
```

The second command answering with an empty history is **success** — it means
Apple accepted the credentials and has no submissions to report yet. An
authentication error here is an authentication error in CI too, and it is much
cheaper to find at this end.

> If `xcrun notarytool` says *tool 'notarytool' requires Xcode*, you have only
> the Command Line Tools. Either install Xcode from the App Store, or point at an
> Xcode that is installed but not selected:
> `export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`.
> (`sudo xcode-select -s …` does it permanently and wants an admin password.)

---

## 7. How PrismDMX is configured to be signed

Three files, and it is worth knowing what each one does before changing any of
them.

### `crates/prism-app/tauri.bundle.macos.conf.json`

The macOS counterpart of `tauri.bundle.conf.json`, passed with `--config`. It
names the bundle targets, the payload and the signing options:

```json
{
  "bundle": {
    "targets": ["app", "dmg"],
    "resources": {
      "../../target/release/prismd": "prismd",
      "../../profiles/surface/xtouch.json": "profiles/surface/xtouch.json",
      "../../profiles/fixtures/": "profiles/fixtures/"
    },
    "macOS": {
      "minimumSystemVersion": "11.0",
      "entitlements": "entitlements.plist"
    }
  }
}
```

**`resources` is the part a signature depends on in a way that is easy to miss,
and S63 missed it first.** `prismd` is a second executable living inside
`PrismDMX.app/Contents/Resources/`. Under the hardened runtime a nested
executable must be signed **by the same team** as the bundle around it, or the
whole thing is refused.

**Tauri does not sign it.** It signs the `.app` and the `.dmg`; an executable it
carried in as a *resource* is data as far as the bundler is concerned. The first
signed bundle built for this document had `prismd` inside it still reading
`flags=0x20002(adhoc, linker-signed)` — which is what the linker leaves behind
and not a signature by anybody. Notarisation rejects that: *the binary is not
signed with a valid Developer ID certificate*.

So the release job signs the engine **before** the bundler copies it, which is
the ordinary inside-out order — the signature travels with the file and Tauri
then seals the app around it:

```bash
codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" target/release/prismd
```

`--options runtime` is the hardened runtime, which notarisation requires of
**every** executable and not only the outer one, and `--timestamp` is a trusted
timestamp, without which the signature stops verifying once the certificate
expires.

> **`codesign --verify` does not catch this, which is why §10 asks for more.**
> An ad-hoc signed binary is *valid on disk* and *satisfies its designated
> requirement*, so a check that only verifies passes on an engine nobody signed.
> What tells the two apart is the **authority** and the **flags**, and the
> release job asks for both.

**There is deliberately no `signingIdentity` in this file.** It comes from the
`APPLE_SIGNING_IDENTITY` environment variable instead, so that the repository
never carries a value that differs between the owner's Mac and CI, and so that a
build with no certificate produces an unsigned bundle rather than an error.

### `crates/prism-app/entitlements.plist`

The hardened runtime's exceptions, and there is one. The file itself argues for
why the others are absent; the rule for adding a new one is that it has to argue
the same way. Each entitlement is a hole in the protection Apple is being asked
to vouch for, and notarisation *does* look at some of them.

### `.github/workflows/release.yml`

The `macos` job. §9.

**What Tauri reads from the environment**, in the `Build the bundle` step:

| Variable | What it does |
|---|---|
| `APPLE_SIGNING_IDENTITY` | The full identity string, e.g. `Developer ID Application: Your Name (ABCDE12345)`. **Empty means do not sign** — which is a supported outcome, not a failure. |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` | Tauri can import a base64 `.p12` itself. The release job imports it into a keychain of its own instead (§9), which is the same work done where it can be cleaned up afterwards. |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Notarisation of the `.app`. All three, or none — two out of three is an error. |

---

## 8. Signing on your own Mac

For a release you do not need this: §9's job is the path that produces what
people download. This is for checking the certificate works, and for a build you
want to hand somebody directly.

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (ABCDE12345)"
```

```bash
cargo build --release -p prismd && cd crates/prism-app && ../../ui/node_modules/.bin/tauri build --config tauri.bundle.macos.conf.json
```

That produces a **signed** `PrismDMX.app` and `.dmg` under
`target/release/bundle/`. To notarise it as well, add the three credentials from
§6 to the same environment before the build:

```bash
export APPLE_ID="you@example.com" APPLE_TEAM_ID="ABCDE12345" APPLE_PASSWORD="abcd-efgh-ijkl-mnop"
```

Tauri notarises the `.app`, but it does **not staple the ticket to the `.dmg`**.
Staple it yourself, or the disk image needs a working internet connection every
time somebody opens it — which in a hall with no network is a file that does not
open:

```bash
xcrun notarytool submit target/release/bundle/dmg/*.dmg --keychain-profile "prismdmx" --wait && xcrun stapler staple target/release/bundle/dmg/*.dmg
```

---

## 9. GitHub Actions: the six secrets and the job that reads them

### 9.1 The secrets

*Settings → Secrets and variables → Actions → New repository secret*, **six
times**. The names are exact — the workflow reads these and no others, and a
missing one is not an error but a **half-configured build**: without the first
three the bundle is unsigned, without the last three it is signed but not
notarised (§1's second state).

| Secret | Value | From |
|---|---|---|
| `APPLE_CERTIFICATE` | The `.p12` from §5.4, **base64 encoded** | the command below |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting it | §5.4 step 5 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Your Name (ABCDE12345)` — the full string, quotes not included | `security find-identity -v -p codesigning` |
| `APPLE_ID` | The Apple ID e-mail of the developer account | §6.1 |
| `APPLE_APP_SPECIFIC_PASSWORD` | `abcd-efgh-ijkl-mnop` | §6.1 |
| `APPLE_TEAM_ID` | `ABCDE12345` | §6.2 |

To produce the base64, **with no line breaks in it** — this is the single most
common way this goes wrong, because `base64` on some systems wraps at 76
characters and the decode in CI then fails on a value that looks perfectly
plausible in the secrets UI:

```bash
base64 -i Certificates.p12 | tr -d '\n' | pbcopy
```

The value is now on your clipboard. Paste it straight into the secret.

### 9.2 What the job does with them

`.github/workflows/release.yml`, job `macos`, in order:

1. **`Is there a certificate`** reads the secrets into `env` and publishes two
   outputs, `signing` and `notarising`. It is a step rather than an `if:` because
   **the `secrets` context is not available to a step's `if:`** — written there,
   `secrets.APPLE_ID != ''` is true on a runner that has no Apple ID at all.
2. **`Import the certificate`** creates a **temporary keychain**, imports the
   `.p12`, sets a one-hour unlock timeout (without it the keychain re-locks
   mid-build and signing fails with a *User interaction is not allowed*), and
   runs `security set-key-partition-list` so `codesign` can use the key with no
   prompt.
3. **`Build the bundle`** runs Tauri with the macOS config and the `APPLE_*`
   variables.
4. **`Verify the signature`** checks the bundle, checks the **hardened runtime
   flag**, and checks the nested `prismd` — see §10 for why each.
5. **`Notarise and staple the disk image`** submits the `.dmg`, waits, staples
   and validates.
6. **`Take the keychain away`** runs `if: always()`, because a private key left
   on a reused runner is exactly the thing that must not survive a failed run.

### 9.3 The rule about `ci.yml`

**This job is in `release.yml` and must never be copied into `ci.yml`.**
`IMPLEMENTATION_PLAN.md` §7 and `CLAUDE.md` keep every push on Linux because
runner minutes are billed per platform and macOS is the dearest — at GitHub's
published rates a macOS minute is **ten times** a Linux one, where Windows is
twice. A push runs Linux; the full pass, Windows and macOS and the installers,
runs on a `v*` tag, or by hand with **Run workflow** (`workflow_dispatch`), which
builds everything and publishes nothing.

---

## 10. Checking the result — the four commands that actually prove it

Run these against the built artefact. "The job went green" is not one of them:
a run can sign nothing at all and exit zero.

**Is it signed, and by whom:**

```bash
codesign --display --verbose=4 target/release/bundle/macos/PrismDMX.app
```

`Authority=Developer ID Application: …`, `Authority=Developer ID Certification
Authority`, `Authority=Apple Root CA` — three lines, in that order. One line
saying `Authority=(unavailable)` or a `Signature=adhoc` is an **unsigned**
bundle wearing a signature's clothes.

**Is the hardened runtime on** — notarisation refuses a bundle without it, and
a signature made without `--options runtime` lacks it silently:

```bash
codesign --display --verbose=2 target/release/bundle/macos/PrismDMX.app 2>&1 | grep flags
```

You want `runtime` in the flags.

**Is everything inside it signed too**, the nested `prismd` included:

```bash
codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/PrismDMX.app
```

**And is the engine signed by *you*** — which the command above does **not**
answer, because an ad-hoc signature verifies perfectly well (§7):

```bash
codesign --display --verbose=2 target/release/bundle/macos/PrismDMX.app/Contents/Resources/prismd
```

`Authority=Developer ID Application: …` and `flags=0x10000(runtime)`. The word
**`adhoc`** anywhere in those flags means the engine was never signed, the
bundle will be rejected by notarisation, and §7 is where the fix is.

**Would Gatekeeper let a stranger open it** — the real question, and the only
one of the four that answers it:

```bash
spctl --assess --type open --context context:primary-signature -vv target/release/bundle/dmg/PrismDMX_0.9.2_aarch64.dmg
```

`source=Notarized Developer ID` and `accepted`. `source=Developer ID` without
*Notarized* means signed but not notarised — state two of §1.

**And the honest test**: download the `.dmg` from the GitHub release page, on a
Mac that has never seen this project, over the internet rather than from a
network share. Only a download carries the `com.apple.quarantine` attribute, and
quarantine is what Gatekeeper reacts to. A file copied off a USB stick opens
whether it is signed or not, and will tell you nothing.

---

## 11. When it goes wrong

| What you see | What it is | What to do |
|---|---|---|
| `security find-identity` lists nothing | The certificate is not in the keychain being searched, or has no private key | §5.3. On CI, the temporary keychain was not made the default or was never unlocked |
| `errSecInternalComponent` when signing | The keychain re-locked, or `codesign` may not use the key | `security set-key-partition-list` (§9.2 step 2). Locally: unlock the login keychain |
| `User interaction is not allowed` | The same thing, on a headless runner: something wanted to show a dialog | as above |
| `The specified item could not be found in the keychain` | `APPLE_SIGNING_IDENTITY` does not match a certificate that is there — usually a truncated string or the wrong Team ID | copy the identity verbatim from `security find-identity -v -p codesigning` |
| `security import`: **`MAC verification failed during PKCS12 import (wrong password?)`** | The password is almost certainly right. The `.p12` was built by **Homebrew OpenSSL 3** without `-legacy`, and macOS cannot read its encryption | rebuild it with `/usr/bin/openssl`, or add `-legacy` (§5.0's warning). This also fails the CI import step, where the message is just as misleading |
| Base64 decode fails in CI | The `.p12` was encoded with line wraps | re-encode with `base64 -i … \| tr -d '\n'` (§9.1) |
| Notarisation: `Team is not yet configured for notarization` | A new account Apple has not finished provisioning | wait; it is usually under an hour. If it persists, Apple Developer Support |
| Notarisation: `Invalid credentials` | The app-specific password was revoked, mistyped, or belongs to a different Apple ID than `APPLE_ID` | §6.1, and test with §6.3 before re-running a release |
| Notarisation rejected, `The executable does not have the hardened runtime enabled` | Signed without `--options runtime` | the identity was empty at build time, so nothing was signed properly. Check the `Is there a certificate` step's log |
| Notarisation rejected, `The binary is not signed with a valid Developer ID certificate` | Usually the **nested engine**, not the app: Tauri does not sign an executable carried in as a resource | §7 — sign `target/release/prismd` before the bundler copies it. Check with `codesign -d --verbose=2 …/Contents/Resources/prismd`; the word `adhoc` in the flags is the symptom |
| `codesign --verify` passes but notarisation still rejects it | An ad-hoc signature is *valid* and *satisfies its designated requirement*; verifying does not ask **who** signed it | ask for the authority and the flags instead of only verifying (§10) |
| Notarisation rejected, `The signature of the binary is invalid` | Something inside the bundle was modified **after** signing | nothing may touch the `.app` between the Tauri build and the `.dmg` |
| `xcrun: error: unable to find utility "notarytool"` | Command Line Tools rather than full Xcode | §6.3's note |
| The `.dmg` opens on your Mac but not on anyone else's | You are testing a file that was never quarantined | §10's last paragraph |
| `"PrismDMX" is damaged and can't be opened` | Unsigned, downloaded, quarantined | this document, from §5 |

To read Apple's own reasons for a rejection — which are far more specific than
the summary — take the submission ID the `notarytool submit` output gave you:

```bash
xcrun notarytool log <submission-id> --keychain-profile "prismdmx"
```

---

## 12. Expiry, revocation and handover

**The certificate lasts five years.** The app-specific password does not expire
but stops working if the Apple ID's password is changed. **The notarisation
ticket never expires** — a build you notarised and shipped keeps opening after
the certificate behind it has expired, which is deliberate on Apple's part and
means an old release does not rot.

Three things are worth doing before they are urgent:

1. **Put a reminder in a calendar** for two months before the certificate's
   expiry date, which `security find-identity -v -p codesigning` and Keychain
   Access both show. Renewal is §5 again, from the beginning — Apple does not
   renew a certificate, you make a new one.
2. **Back up the `.p12` and its password** somewhere that is not this Mac and
   not this repository. If the private key is lost, every future build needs a
   new certificate; on an organisation account that also means spending one of
   a limited number of Developer ID slots.
3. **Write down who the Account Holder is.** This is a project for schools and
   venues, and the most likely failure in three years is not technical: it is
   that the one person who could create a certificate has left, and nobody
   knows whose Apple ID the account is under.

**If the key is ever exposed**, revoke the certificate at
*Certificates, Identifiers & Profiles* immediately and issue a new one. Software
already signed and notarised keeps working — revocation is not retroactive
unless Apple additionally blocks the ticket — but anything signed with it after
the exposure is signed in your name by somebody else.

---

## See also

- `docs/BUILD_INSTALLER.md` — the Windows installer, built by hand
- `docs/manual/developer.en.md` §8 — every gate, and which platform answers it
- `IMPLEMENTATION_PLAN.md` §7 — why CI is Linux-only and this job is not in it
- `ARCHITECTURE_SPEC.md` §10.1 — where platform code is allowed to live
