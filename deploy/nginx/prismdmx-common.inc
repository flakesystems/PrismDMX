# Was für beide Namen gilt.

# --- Kopfzeilen -----------------------------------------------------------
#
# Zwei statische Seiten brauchen wenig, aber das Wenige ist billig.
add_header X-Content-Type-Options "nosniff"                       always;
add_header Referrer-Policy        "strict-origin-when-cross-origin" always;
add_header X-Frame-Options        "SAMEORIGIN"                    always;

# Die Grenze, die diese beiden Seiten ohnehin einhalten, als Regel formuliert:
# nichts wird von irgendwo nachgeladen. Kein CDN, keine Schriftart von einem
# fremden Server, keine Zählpixel. Wer das später ändert, merkt es hier — und
# das ist die richtige Stelle, es zu merken.
#
# `unsafe-inline` für Stile, weil `prism-web` sein Stylesheet in die Seite
# einbettet (eine Handbuchseite soll eine Anfrage sein). Skripte brauchen es
# nicht: `prism-site` liefert `site.js` als eigene Datei aus, und `prism-web`
# hat gar keins.
add_header Content-Security-Policy "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; script-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'self'" always;

# --- Zwischenspeicher -----------------------------------------------------
#
# **`expires` und nicht `add_header Cache-Control`**, und das ist kein Stil,
# sondern die eine nginx-Falle, die hier zuschlägt: ein `add_header` in einem
# `location` wirft *alle* geerbten `add_header` der Ebene darüber weg. Die vier
# Kopfzeilen oben wären für genau die Dateien verschwunden, für die man den
# Cache einstellen wollte. `expires` ist eine andere Direktive und erbt nicht
# in dieser Weise.
#
# Die Beilagen tragen keinen Hash im Namen, also darf ihr Cache nicht ewig
# sein: eine neue Fassung soll ankommen, ohne dass jemand Strg-F5 drückt. Eine
# Stunde ist der Kompromiss — lang genug, dass ein Besuch nichts nachlädt, kurz
# genug, dass ein Deployment am selben Tag sichtbar ist.
location ~* \.(css|js|svg|woff2?|png|jpg|webp)$ {
    expires 1h;
}

# HTML nie. Es ist klein, und es ist das Einzige, was sagt, welche Version
# gerade gilt.
location ~* \.html$ {
    expires -1;
}

# --- Kleinkram ------------------------------------------------------------
gzip on;
gzip_vary on;
gzip_min_length 1024;
gzip_types text/plain text/css text/html application/javascript image/svg+xml application/json;

server_tokens off;

# Ein Browser fragt nach `/favicon.ico`, ob die Seite eine anbietet oder nicht.
# Die Startseite liefert eine — das Zeichen, das auch das Pult trägt —, die
# Handbücher nicht.
#
# **`try_files` und nicht `return 204`**, und das ist eine Korrektur: die Regel
# stand hier als glattes `return 204`, aus der Zeit, in der die Startseite ihr
# Symbol nur als SVG führte. Seit sie eine echte `.ico` ausliefert, verschluckte
# genau diese Regel sie — die Datei lag im Verzeichnis, nginx antwortete
# trotzdem mit *kein Inhalt*, und im Reiter blieb das Symbol leer. Gefunden hat
# das kein Test, sondern ein `curl` gegen den laufenden Server.
#
# So bekommt die Startseite ihre Datei, und die Handbücher geben weiterhin 204
# statt einer 404-Zeile im Log.
location = /favicon.ico { try_files $uri =204; access_log off; }
