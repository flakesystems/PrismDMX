# prismdmx.de und docs.prismdmx.de, in einem Container.
#
# Zwei Stufen, und die Trennung ist der Punkt: bauen braucht eine
# Rust-Toolchain und das ganze Repository, ausliefern braucht **nichts** außer
# ein paar Dateien und einem Webserver. Was am Ende läuft, ist ein nginx mit
# statischen Dateien darin — kein Rust, kein Node, keine Datenbank, nichts, was
# man patchen müsste, weil ein Angreifer es erreichen könnte.
#
# Warum beide Seiten in **einem** Image liegen:
#
#   - Sie werden aus **einem** Commit gebaut. Eine Startseite, die 0.9.2
#     bewirbt, während die Handbücher 0.9.1 beschreiben, ist genau die Sorte
#     Abweichung, gegen die `prism-web` überhaupt existiert. Ein Image ist ein
#     Stand von beidem.
#   - Zwei Container wären zwei Deployments und ein Zeitfenster dazwischen.
#   - Was sie trennt, ist ohnehin nur ein `server_name`, und den kann ein nginx.
#
# Bauen und starten:
#
#     docker compose -f deploy/docker-compose.yml up -d --build
#
# **Der Server holt, er wird nicht beschickt.** SSH auf diesen Server geht nur
# über VPN, also kann kein Build-Runner hineinschieben, und die Antwort darauf
# ist nicht, dafür ein Loch aufzumachen: der Server macht `git pull` und baut
# selbst. Ausgehend, kein eingehender Port, kein Deploy-Schlüssel bei GitHub.
# `deploy/README.md` hat das als Ablauf, und `ARCHITECTURE_SPEC.md` §10 hält
# dieselbe Überlegung für die GitHub-Pages-Variante fest.

# ---------------------------------------------------------------------------
# Stufe 1 — die beiden Generatoren laufen lassen
# ---------------------------------------------------------------------------
# Die Version ist die aus `rust-toolchain.toml`; `[workspace.package]` verlangt
# mindestens 1.89, weil `prismd` `File::try_lock` braucht.
FROM rust:1.89-bookworm AS build

WORKDIR /repo

# Erst die Manifeste, dann alles andere: so bleibt die Abhängigkeitsauflösung
# in einer Ebene, die sich nicht ändert, wenn jemand einen Satz im Handbuch
# umschreibt. Der Workspace nennt `crates/*`, also müssen alle Manifeste da
# sein — gebaut werden mit `-p` trotzdem nur zwei Pakete.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY web ./web
COPY site ./site

RUN cargo build --release --locked -p prism-site -p prism-web

# Jetzt der Inhalt. `prism-web` liest die Markdown-Dateien zur Laufzeit, also
# muss das Repository hier stehen, wenn der Generator läuft — anders als
# `prism-site`, dessen Vorlage einkompiliert ist.
COPY docs ./docs
COPY README.md CHANGELOG.md ARCHITECTURE_SPEC.md IMPLEMENTATION_PLAN.md CLAUDE.md LICENSE ./

# Das `CNAME` und das `.nojekyll` sind für GitHub Pages und hier bedeutungslos.
# Sie liegen zu lassen wären zwei Dateien, die etwas Falsches behaupten.
#
# Der Kommentar steht **über** dem `RUN` und nicht darin: eine Zeile hinter
# einer Fortsetzung gehört zur selben Shell-Zeile, und ein Doppelkreuz dort
# kommentiert alles dahinter weg — einschließlich des `rm`.
RUN set -eu; \
    ./target/release/prism-site --out /out/www; \
    ./target/release/prism-web  --out /out/docs; \
    rm -f /out/www/CNAME /out/docs/CNAME /out/www/.nojekyll /out/docs/.nojekyll

# ---------------------------------------------------------------------------
# Stufe 2 — ausliefern
# ---------------------------------------------------------------------------
FROM nginx:1.27-alpine AS serve

# Die mitgelieferte Vorgabe zeigt auf eine Willkommensseite und würde für einen
# unbekannten `Host` antworten. Weg damit; `prismdmx.conf` bringt ihre eigene
# Vorgabe mit, die 404 sagt.
RUN rm -f /etc/nginx/conf.d/default.conf

# Das ganze Verzeichnis: nginx lädt aus `conf.d` von selbst jede `*.conf`,
# und die `.inc` daneben wird von der `.conf` eingebunden statt selbst
# geladen zu werden — sonst stünden ihre Direktiven zweimal im Baum.
COPY deploy/nginx/ /etc/nginx/conf.d/
COPY --from=build /out/www  /srv/prismdmx/www
COPY --from=build /out/docs /srv/prismdmx/docs

# Kein TLS in diesem Container, und das ist Absicht: ein Zertifikat gehört an
# **eine** Stelle vor allen Diensten dieses Servers und nicht in jedes Image.
# Davor steht ein Reverse Proxy — Caddy, Traefik, nginx-proxy-manager —, der
# terminiert und hierher weiterreicht. `deploy/README.md` hat die drei Zeilen
# dafür.
EXPOSE 80

# Ein Container, der beim Neustart des Servers wieder hochkommt und der sagt,
# ob er gesund ist. `wget` liegt im Alpine-Basisimage.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q -O /dev/null --header="Host: prismdmx.de" http://127.0.0.1/ || exit 1
