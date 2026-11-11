#!/bin/sh
# Lists the mounted game files (/srv/game) for the page as [[path, size], ...] in /tmp/game-index.json (ADR 0081);
# an empty or missing folder gives [], and the page then only offers the player's own install.
set -eu
out=/tmp/game-index.json
if [ -d /srv/game ]; then
  cd /srv/game
  find . -type f -exec stat -c '%s %n' {} + | sort -k2 | awk '
    BEGIN { printf "["; n = 0 }
    {
      size = $1; path = substr($0, length($1) + 4)  # past "SIZE ./"
      # Files at the top that the client never reads (the same list as tools/serve_web.py SKIP).
      p = tolower(path)
      if (p !~ /\// && (p == "ig2" || p ~ /\.(dll|so|lnk|ico|pdf)$/ || p ~ /\.so\./ || (p ~ /\.exe$/ && p != "ig2.exe") \
          || p ~ /^(unins|goggame|language_setup|steam_api)/ || p ~ /^(webcache\.zip|goglog\.ini|app\.id|eula\.txt)$/)) next
      gsub(/\\/, "\\\\", path); gsub(/"/, "\\\"", path)
      printf "%s[\"%s\",%s]", (n++ ? "," : ""), path, size
    }
    END { print "]" }' > "$out"
else
  echo '[]' > "$out"
fi
echo "game-index: $(grep -o '",[0-9]*\]' "$out" | wc -l) game files listed"
