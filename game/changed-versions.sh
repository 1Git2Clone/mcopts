#!/usr/bin/env bash
# Versions for CI to verify. Never all 17 (that's a local game/verify.sh):
# a changed data/defaults/<v>.txt verifies <v>; anything else, the oldest and
# newest, which cover both launch paths (JDK 21 + GLFW, JDK 25 + SDL/EGL).
set -eu
base=${1-}
head=${2:-HEAD}
ends() {
  printf '%s\n' data/defaults/*.txt | sed 's|.*/||;s/\.txt$//' | sort -V | sed -n '1p;$p'
}
# A new branch's push gives no usable base (empty, zeros, or the head itself).
if [ -z "$base" ] || [ "$(git rev-parse -q --verify "$base^{commit}")" = "$(git rev-parse "$head")" ] ||
  ! git cat-file -e "$base^{commit}" 2>/dev/null; then
  ends
else
  changed=$(git diff --name-only --no-renames "$base" "$head")
  if grep -qE '^(game|tests/fixtures)/' <<<"$changed"; then
    ends
  else
    while IFS= read -r path; do
      case $path in
        data/defaults/*.txt)
          if [ -f "$path" ]; then basename "$path" .txt; fi ;;
      esac
    done <<<"$changed" | sort -Vu
  fi
fi
