#!/usr/bin/env bash
# Versions to verify for a push; no base (manual/initial push) means all.
set -eu
base=${1-}
head=${2:-HEAD}
all() { printf '%s\n' data/defaults/*.txt | sed 's|.*/||;s/\.txt$//' | sort -V; }
if [ -z "$base" ] || ! git cat-file -e "$base^{commit}" 2>/dev/null; then
  all
else
  changed=$(git diff --name-only --no-renames "$base" "$head")
  if grep -qE '^(game|tests/fixtures)/' <<<"$changed"; then
    all
  else
    while IFS= read -r path; do
      case $path in
        data/defaults/*.txt)
          if [ -f "$path" ]; then basename "$path" .txt; fi ;;
      esac
    done <<<"$changed" | sort -Vu
  fi
fi
