#!/usr/bin/env bash
# verify.sh [version...]
#
# Proves mcopts against the real game, one version at a time: `mcopts apply`
# hutao's options ($MCOPTS_OPTIONS, tests/fixtures/hutao: a copy of hutao/vps
# packs/options/all/options.txt) into a fresh
# game folder, `mcopts check` it, launch, and require that the game loads it
# (no "Failed to load options") and re-saves the vanilla options we wrote.
# Unknown keybinds need their mods to survive a save. Then these modern binds
# without version: must log "Failed to load options". Run it
# from the `game` dev shell.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
cache=${MCOPTS_GAME_CACHE:-$HOME/.cache/mcopts-game}
options=${MCOPTS_OPTIONS:-$here/../tests/fixtures/hutao}
versions=${*:-$(printf '%s\n' "$here"/../data/defaults/*.txt | sed 's|.*/||;s/\.txt$//' | sort -V)}
mcopts() { cargo run -q --manifest-path "$here/../Cargo.toml" -- "$@"; }
# Options, then Escape: saves even in 1.21, where F11 twice does not.
# Coordinates are for hutao's fullscreen:true, guiScale:2 on 1280x720 Xvfb.

failed=0
fail() {
  echo "FAIL $v: $*" >&2
  failed=1
  bad=1
}
for v in $versions; do
  bad=0
  case $v in
    26.3) steps="key,F2 click,326,428 key,F2 key,Escape" ;;
    26.2) steps="key,F2 click,540,488 key,F2 key,Escape" ;;
    *) steps="key,F2 click,540,464 key,F2 key,Escape" ;;
  esac
  dir=$cache/runs/verify-$v
  rm -rf "$dir" "$dir-bare"
  mcopts apply -m "$v" "$dir" "$options" || { fail "apply"; continue; }
  mcopts check -m "$v" "$dir/options.txt" || { fail "check"; continue; }
  cp "$dir/options.txt" "$cache/runs/wrote-$v.txt"
  # Vanilla drops mod binds and binds only other versions know.
  awk -F: 'NR == FNR { known[$1]; next } !/^key_/ || $1 in known' \
    "$here/../data/defaults/$v.txt" "$cache/runs/wrote-$v.txt" >"$dir/expected.txt"
  sleep 1
  MCOPTS_STEPS="$steps" "$here/launch.sh" "$v" "$dir" || { fail "launch"; continue; }
  if grep -q "Failed to load options" "$dir/logs/latest.log"; then
    fail "the game rejected what we wrote"
  elif ! [ "$dir/options.txt" -nt "$cache/runs/wrote-$v.txt" ]; then
    fail "the game did not re-save options.txt"
  elif ! mcopts diff "$dir/expected.txt" "$dir/options.txt"; then
    fail "the game changed or dropped the options above"
  fi

  mkdir -p "$dir-bare"
  grep -v '^version:' "$cache/runs/wrote-$v.txt" >"$dir-bare/options.txt"
  MCOPTS_STEPS='' "$here/launch.sh" "$v" "$dir-bare" || { fail "launch without version:"; continue; }
  grep -q "Failed to load options" "$dir-bare/logs/latest.log" ||
    fail "loaded options.txt without version: (expected Failed to load options)"
  if [ "$bad" -eq 0 ]; then echo "ok $v"; fi
done
exit $failed
