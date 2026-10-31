#!/usr/bin/env bash
# collect.sh [version...]
#
# Launches each release fresh, one at a time, and keeps the options.txt it
# writes as data/defaults/<version>.txt. 1.21.x writes nothing until the
# player acts, so every version gets the same first move: Continue on the
# first-launch accessibility screen (which sets onboardAccessibility:false).
# A screenshot taken before the click, in the run's screenshots/, shows what
# was clicked. Run it from the `game` dev shell.
set -eu

here=$(dirname "$0")
cache=${MCOPTS_GAME_CACHE:-$HOME/.cache/mcopts-game}
versions=${*:-1.21 1.21.1 1.21.2 1.21.3 1.21.4 1.21.5 1.21.6 1.21.7 1.21.8 \
  1.21.9 1.21.10 1.21.11 26.1 26.1.1 26.1.2 26.2 26.3}

for v in $versions; do
  dir=$cache/runs/defaults-$v
  rm -rf "$dir"
  # ponytail: fixed click on Continue in the default 854x480 window; read the
  # screenshot if a version moves it.
  MCOPTS_STEPS="key,F2 click,426,447" "$here/launch.sh" "$v" "$dir"
  grep -q '^version:' "$dir/options.txt"
  cp "$dir/options.txt" "$here/../data/defaults/$v.txt"
done
