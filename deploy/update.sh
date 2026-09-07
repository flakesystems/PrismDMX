#!/bin/sh
# prismdmx.de neu bauen und austauschen — für den LXC ohne Docker.
#
# Der Server **holt**: dieses Skript läuft auf ihm, zieht den Stand und baut
# ihn bei sich. Kein eingehender Port, kein Deploy-Schlüssel bei GitHub.
# `deploy/README.md` §4 hat die Begründung, und sie ist eine Einschränkung des
# Eigentümers und keine Vorliebe: SSH hierher geht nur über VPN.
#
# Aufruf, von Hand oder aus einem systemd-Timer:
#
#     /opt/prismdmx/deploy/update.sh
#
# `set -e`, weil ein halb ausgetauschtes Web-Verzeichnis schlimmer ist als ein
# altes: bricht irgendetwas ab, bleibt das Ausgelieferte, was es war.
set -eu

REPO="${PRISMDMX_REPO:-/opt/prismdmx}"
ROOT="${PRISMDMX_ROOT:-/srv/prismdmx}"

cd "$REPO"

# `--ff-only`: was hier steht, ist eine Kopie und keine Arbeitskopie. Ein Merge
# auf einem Server wäre ein Zustand, den niemand ansieht.
git fetch --quiet origin
git merge --ff-only --quiet origin/master

# Nach `dist/` neben dem Repository und nicht direkt ins Web-Verzeichnis: ein
# Build, der auf halber Strecke abbricht, soll das Ausgelieferte nicht angefasst
# haben.
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

cargo build --release --locked -p prism-site -p prism-web
./target/release/prism-site --out "$STAGE/www"
./target/release/prism-web  --out "$STAGE/docs"

# Das `CNAME` und das `.nojekyll` sind für GitHub Pages und hier bedeutungslos.
rm -f "$STAGE/www/CNAME" "$STAGE/docs/CNAME" "$STAGE/www/.nojekyll" "$STAGE/docs/.nojekyll"

# Austauschen. `mv` über eine Verzeichnisgrenze wäre ein Kopiervorgang mit einem
# sichtbaren Zwischenzustand, also erst danebenlegen, dann umbenennen — beides
# auf demselben Dateisystem, damit das Umbenennen ein Umbenennen ist.
mkdir -p "$ROOT"
for name in www docs; do
    cp -a "$STAGE/$name" "$ROOT/.$name.new"
    rm -rf "$ROOT/.$name.old"
    if [ -d "$ROOT/$name" ]; then mv "$ROOT/$name" "$ROOT/.$name.old"; fi
    mv "$ROOT/.$name.new" "$ROOT/$name"
    rm -rf "$ROOT/.$name.old"
done

# nginx liefert statische Dateien aus und hält keine Verzeichnis-Handles über
# einen Austausch hinweg, aber ein Reload kostet nichts und räumt die
# Deskriptoren auf, die noch auf das alte Verzeichnis zeigen.
if command -v nginx >/dev/null 2>&1; then
    nginx -t && nginx -s reload
fi

echo "prismdmx.de neu gebaut: $(git rev-parse --short HEAD)"
