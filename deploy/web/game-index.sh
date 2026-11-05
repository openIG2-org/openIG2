#!/bin/sh
# Lists the mounted game files (/srv/game) for the page as [[path, size], ...] in /tmp/game-index.json (ADR 0081);
# an empty or missing folder gives [], and the page then only offers the player's own install.
set -eu
out=/tmp/game-index.json
if [ -d /srv/game ]; then
  cd /srv/game
  find . -type f -exec stat -c '%s %n' {} + | sort -k2 | awk '
    BEGIN { printf "[" }
    {
      size = $1; path = substr($0, length($1) + 4)  # past "SIZE ./"
      gsub(/\\/, "\\\\", path); gsub(/"/, "\\\"", path)
      printf "%s[\"%s\",%s]", (NR > 1 ? "," : ""), path, size
    }
    END { print "]" }' > "$out"
else
  echo '[]' > "$out"
fi
echo "game-index: $(grep -o '",[0-9]*\]' "$out" | wc -l) game files listed"
