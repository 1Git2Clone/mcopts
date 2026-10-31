# mcopts

A library for managing your Minecraft options. It sounds ridiculous until
you try to.

Read, check, merge and sync Minecraft's `options.txt` and mod configs, for
Minecraft 1.21 to 26.3.

With the tested modern keybinds, every supported client logs `Failed to load
options` and falls back to defaults if `version:` is missing. This does not
mean every versionless file fails: legacy files can be migrated. Unknown
options are dropped when the game saves. mcopts knows each version's options
from what the game writes (`data/defaults/<version>.txt`); `check` validates
keys and value kinds, not every range or enum. It warns about the graphics
preset interactions observed below; warnings do not change the exit status.
Mod keybinds are allowed for modded instances, but vanilla drops them on save.

```sh
mcopts versions                         # name, data version, option count
mcopts check -m 1.21.1 options.txt      # check version, keys and value kinds
mcopts apply -m 1.21.1 out/ dir1 dir2   # lay option dirs over out/, in order
mcopts sync instance/ dir               # take an instance's preferences
mcopts diff a.txt b.txt                 # options in a that b lacks or changes
```

An option directory mirrors the game folder: its `options.txt` merges key by
key and gets the version's `version:`, and every other file (mod configs)
replaces the target's. `sync` leaves out configs that are not a player's
preferences (`data/not-preferences.txt`).

## Proving it against the game

`game/` launches real clients offline on Xvfb, never on the desktop, one at a
time, silently, each held to 2 GiB of heap. It inspects screenshots, logs and
the `options.txt` the game writes:

```sh
nix develop .#game
game/collect.sh        # data/defaults/<v>.txt from a fresh launch of each
game/verify.sh         # apply + check hutao's options, launch, compare
```

Collection clicks Continue on the first-launch accessibility screen; quitting
alone does not save a fresh 1.21.x instance's options. Verification opens Options and leaves with
Escape, and checks that a save occurred: F11 twice did not save in 1.21.
Only keybinds present in that version's vanilla defaults are compared; mod
binds remain in the applied file but require their mods to survive a save.

In 1.21.11, 26.1, 26.1.1, 26.1.2, 26.2 and 26.3, `graphicsPreset:"fancy"`
overwrote `entityDistanceScaling:0.75` and `simulationDistance:6` with `1.0`
and `12`. Use `graphicsPreset:"custom"` to preserve individual settings.
`check` and `apply` warn when a non-custom preset accompanies either of those
keys in those versions. Verification still fails on changed vanilla values.

Game files are cached in `~/.cache/mcopts-game`.

Forgejo CI runs `nix flake check` and `cargo test` on every push, including
tags. A separate game workflow verifies the versions whose defaults changed;
for anything else (`game/` or the options fixture changed, a manual run) it
verifies the oldest and newest, which cover both launch paths. All 17 is a
local `game/verify.sh`. It uses `tests/fixtures/hutao`, a copy of hutao/vps's options (vps
checks its live ones with `mcopts check` in its own build), and caches game
downloads when the runner supports caching.
The game workflow does not gate releases.

## Development

```sh
nix develop     # cargo, clippy, rustfmt, pre-commit hooks
cargo test
nix build
```
