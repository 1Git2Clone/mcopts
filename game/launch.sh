#!/usr/bin/env bash
# launch.sh <version> <game dir> [seconds]
#
# Starts a Minecraft client offline on a virtual X display, never the desktop,
# waits until it has loaded, closes its window as a player would (window.py), and waits
# for it to exit. One game at a time, each held to 2 GiB of heap; anything
# left over is killed with its whole process group. Run it from the `game`
# dev shell (nix develop .#game), which provides Java, portablemc and Xvfb.
#
# Game files are downloaded once into $MCOPTS_GAME_CACHE (~/.cache/mcopts-game).
set -u

cache=${MCOPTS_GAME_CACHE:-$HOME/.cache/mcopts-game}

if [ "${1-}" = --inside ]; then
  # Inside xvfb-run: start the game, wait for it, close it.
  shift
  v=$1 dir=$2 secs=$3 java=$4
  portablemc --main-dir "$cache/main" start --mc-dir "$dir" "$v" -u mcopts \
    --jvm "$java" --jvm-arg=-Xmx2G --disable-multiplayer --disable-chat &
  game=$!
  for _ in $(seq "$secs"); do
    sleep 1
    grep -q "Created: .*atlas" "$dir/logs/latest.log" 2>/dev/null && break
    kill -0 $game 2>/dev/null || exit 1
  done
  grep -q "Created: .*atlas" "$dir/logs/latest.log" 2>/dev/null || exit 1
  sleep 5
  for step in ${MCOPTS_STEPS-}; do
    IFS=, read -r -a args <<<"$step"
    python3 "$(dirname "$0")/window.py" "${args[@]}" || exit 1
    sleep 1
  done
  python3 "$(dirname "$0")/window.py" close || exit 1
  for _ in $(seq 60); do
    kill -0 $game 2>/dev/null || exit 0
    sleep 1
  done
  exit 1
fi

v=$1
dir=$(realpath -m "$2")
secs=${3:-300}
ours() {
  for p in $(pgrep java); do
    tr '\0' ' ' </proc/"$p"/cmdline 2>/dev/null | grep -q -- "$cache" && return 0
  done
  return 1
}
if ours; then
  echo "a test game is still running; not starting another" >&2
  exit 1
fi
case $v in
  1.21*) java=$JDK21 ;;
  *) java=$JDK25 ;;
esac
mkdir -p "$dir"
rm -f "$dir/logs/latest.log"
# No WAYLAND_DISPLAY: GLFW prefers Wayland, and would open on the desktop.
# Silent too: OpenAL gets its null device, and without flite the narrator
# (which reads the first-launch screen aloud) has no voice.
LD_LIBRARY_PATH=$(tr : '\n' <<<"${LD_LIBRARY_PATH-}" | grep -v flite | paste -sd:)
setsid env -u WAYLAND_DISPLAY -u PULSE_SERVER -u PIPEWIRE_REMOTE \
  XDG_SESSION_TYPE=x11 LIBGL_ALWAYS_SOFTWARE=1 ALSOFT_DRIVERS=null SDL_AUDIO_DRIVER=dummy \
  SDL_VIDEO_DRIVER=x11 SDL_VIDEO_FORCE_EGL=1 \
  LD_LIBRARY_PATH="$LD_LIBRARY_PATH" \
  xvfb-run -a -s "-screen 0 1280x720x24" "$0" --inside "$v" "$dir" "$secs" "$java" \
  >"$dir/launch.log" 2>&1 &
group=$!
wait $group
status=$?
kill -KILL -- -$group 2>/dev/null
sleep 1
if ours; then
  echo "a test game survived its process group being killed" >&2
  exit 1
fi
if [ $status -ne 0 ]; then
  echo "$v: did not load and quit cleanly; see $dir/launch.log" >&2
  exit 1
fi
echo "$v: quit cleanly; options.txt: $(test -f "$dir/options.txt" && grep -c : "$dir/options.txt" || echo none)"
