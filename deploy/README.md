# `deploy/` — prismdmx.de auf dem eigenen Server

Zwei Seiten, ein Container:

| Name | Was | Woraus |
|---|---|---|
| `prismdmx.de` | die Startseite | [`site/`](../site/README.md) — `prism-site` |
| `docs.prismdmx.de` | die Handbücher | [`web/`](../web/README.md) — `prism-web` |

Beide werden aus **einem** Commit erzeugt, also können sie nicht auseinander
laufen. Eine Startseite, die `0.9.2` bewirbt, während die Handbücher `0.9.1`
beschreiben, ist genau die Abweichung, gegen die `prism-web` überhaupt gebaut
wurde.

---

## 1. Die kurze Fassung

Auf dem Server, einmal:

```bash
git clone https://github.com/flakesystems/PrismDMX.git /opt/prismdmx
cd /opt/prismdmx
docker compose -f deploy/docker-compose.yml up -d --build
```

Danach hört der Container auf `127.0.0.1:8080` und unterscheidet die beiden
Seiten am `Host`-Kopf. Prüfen, ohne DNS:

```bash
curl -sI -H 'Host: prismdmx.de'      http://127.0.0.1:8080/ | head -1
curl -sI -H 'Host: docs.prismdmx.de' http://127.0.0.1:8080/ | head -1
```

Aktualisieren:

```bash
cd /opt/prismdmx && git pull && docker compose -f deploy/docker-compose.yml up -d --build
```

---

## 2. Der Reverse Proxy davor

**Im Container ist kein TLS**, und das ist Absicht: ein Zertifikat gehört an
eine Stelle vor allen Diensten dieses Servers und nicht in jedes Image. Was
davor steht, terminiert und reicht durch — der `Host`-Kopf muss dabei erhalten
bleiben, weil er das Einzige ist, woran der nginx die beiden Seiten
unterscheidet.

**Caddy** — vollständig, einschließlich Zertifikat:

```caddyfile
prismdmx.de, www.prismdmx.de {
	reverse_proxy 127.0.0.1:8080
}

docs.prismdmx.de {
	reverse_proxy 127.0.0.1:8080
}
```

**nginx** als Proxy:

```nginx
server {
    listen 443 ssl;
    server_name prismdmx.de www.prismdmx.de docs.prismdmx.de;
    # ssl_certificate … (certbot)

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host              $host;   # ← das Entscheidende
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
    }
}
```

**Traefik** über Labels: dann in der `docker-compose.yml` die `ports` streichen,
beide Container in dasselbe Netz hängen (das auskommentierte `networks:` am
Ende der Datei) und dem Dienst
`traefik.http.routers.prismdmx.rule=Host(``prismdmx.de``) || Host(``docs.prismdmx.de``)`
geben.

### DNS

Zwei Einträge auf die Adresse dieses Servers, mehr nicht:

```
prismdmx.de.        A     <IP>
www.prismdmx.de.    CNAME prismdmx.de.
docs.prismdmx.de.   CNAME prismdmx.de.
```

---

## 3. Ohne Docker: ein LXC mit nginx

Wenn der Container ein LXC sein soll und kein Docker darin laufen soll, ist es
derselbe nginx, nur ohne Image. Im Container, einmal:

```bash
apt install -y nginx git build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
git clone https://github.com/flakesystems/PrismDMX.git /opt/prismdmx
```

Ein Skript, das baut und austauscht — `/opt/prismdmx/deploy/update.sh` ist
genau das und liegt hier bei:

```bash
/opt/prismdmx/deploy/update.sh
```

Und die nginx-Konfiguration ist dieselbe wie im Image:

```bash
cp /opt/prismdmx/deploy/nginx/prismdmx.conf        /etc/nginx/conf.d/
cp /opt/prismdmx/deploy/nginx/prismdmx-common.inc  /etc/nginx/conf.d/
nginx -t && systemctl reload nginx
```

Ein Timer, der einmal am Tag nachsieht, ob es etwas Neues gibt:

```ini
# /etc/systemd/system/prismdmx-web.service
[Unit]
Description=prismdmx.de neu bauen
[Service]
Type=oneshot
ExecStart=/opt/prismdmx/deploy/update.sh
```

```ini
# /etc/systemd/system/prismdmx-web.timer
[Unit]
Description=prismdmx.de täglich nachziehen
[Timer]
OnCalendar=daily
RandomizedDelaySec=1h
Persistent=true
[Install]
WantedBy=timers.target
```

```bash
systemctl enable --now prismdmx-web.timer
```

---

## 4. Warum der Server holt und nicht beschickt wird

**SSH auf diesen Server geht nur über VPN.** Ein Build-Runner bei GitHub kann
also nicht `rsync`en, und die Antwort darauf ist ausdrücklich *nicht*, dafür
einen Port aufzumachen oder einen Deploy-Schlüssel bei GitHub zu hinterlegen.

Deshalb dreht dieses Verzeichnis die Richtung um: der Server **holt** — ein
`git pull` und ein Build bei sich, oder (wenn es einmal ein Release-Asset gibt)
ein `curl` über ausgehendes HTTPS mit einer Prüfsumme dagegen. Ausgehend, kein
eingehender Port, kein Schlüssel bei einem Dritten.

Das ist dieselbe Eigenschaft, die die GitHub-Pages-Variante heute hat, und sie
ist es wert, beim Umzug nicht verloren zu gehen.
`.github/workflows/pages.yml` und `ARCHITECTURE_SPEC.md` §10 halten dieselbe
Überlegung von der anderen Seite fest.

---

## 5. Was noch bei GitHub Pages liegt

`.github/workflows/pages.yml` baut und veröffentlicht weiterhin **die
Handbücher** auf `docs.prismdmx.de` — der Generator schreibt das `CNAME`
selbst, seit S55 mit dem `docs.`-Label. Das ist kein Widerspruch zu diesem
Verzeichnis, sondern eine Wahl:

- **Nur der eigene Server.** Dann bleibt Pages ungenutzt; der Workflow schadet
  nicht, und sein Deploy-Schritt ist das Einzige im Repository, das fehlschlägt,
  solange Pages für das Repository nicht eingeschaltet ist — und er schlägt
  **allein** fehl, weil `ci.yml` die Seite baut, ohne sie zu veröffentlichen.
- **Beides, mit Pages als Ausweichweg.** Dann zeigt `docs.prismdmx.de` auf
  Pages und die Startseite auf den eigenen Server; die Seiten wissen nichts
  voneinander, weil jede nur relative Adressen erzeugt und die Startseite die
  Adresse der Handbücher als eine Konstante trägt (`prism_site::DOCS`, mit
  `--docs` überschreibbar).

Was **nicht** geht, ist beides bei Pages aus diesem Repository: GitHub Pages
bedient pro Repository genau eine Domain.

---

## 6. Nachsehen, was gebaut wurde

Die Seiten lassen sich auch ohne Container nebeneinanderlegen:

```bash
cargo run -p prism-site -- --out /tmp/www --docs http://localhost:8081
cargo run -p prism-web  -- --out /tmp/docs
python3 -m http.server 8080 -d /tmp/www &
python3 -m http.server 8081 -d /tmp/docs &
```

`--docs` ist dafür da: die Startseite verlinkt sonst die echte Domain, und ein
Link, der beim Ausprobieren woandershin führt, ist ein Link, den niemand
ausprobiert.
