# OpenSnake (Rust)

OpenSnake rewritten in Rust with [macroquad](https://macroquad.rs): the same game, faster and nicer to look at, on desktop and in the browser.

It plays on the same servers as the Python game. Your account, snake points, skins, ranks and records all carry over, and speedruns from either game count on the same leaderboards.

## What's in it

- **Play**: every mode from the original (Wall, Portal, Cheese, Borderless, Twin, Yin Yang, Key, Sokoban, Poison, Dimension, Minesweeper, Statue, Light, Shield, Arrow, Magnet, Gate), mixed however you like.
- **3D view**: a fixed camera over the board, for every mode, including speedruns and versus.
- **Speedruns**: the built-in categories, this week's ten weekly ones and community categories, with PBs, WRs and leaderboards. Speedrun your own settings to make a community category.
- **Online versus**: ranked or unranked, public or private, with bots, invites and lag compensation.
- **Store**: the shop, loot boxes, a player market for selling spare skins and packs (you can own more than one copy), and your collection.
- **Battle pass**: claim this week's rewards as you level up. It includes pass-only skins with their own animated patterns.
- **Daily challenges**: three a day for snake points, plus a bonus for finishing all three.
- **Ranks**: rewards the first time you reach each rank, including ranked-only skins.
- **Profile**: your picture, bio, friends and messages. The community leaderboard ranks players by a score out of 100.

## Building

You need [Rust](https://rustup.rs).

```sh
cargo run --release                 # play on desktop
cargo test --release                # checks the game matches the Python one exactly
./web/build-web.sh                  # the browser version, in web/dist
```

On Linux, the desktop build needs the ALSA development files (`libasound2-dev` on Debian and Ubuntu).

To serve the browser version from an OpenSnake server, copy `web/dist` to the server's `web-support/rs/`. It becomes the site's main game at `/`, and the Python game moves to `/py/`.

## How it stays in sync with the Python game

The server checks every speedrun by replaying it in Python, so this game's rules have to match exactly:

- `src/pyrandom.rs` is CPython's random number generator.
- `src/game.rs` is a line-by-line port of `game.py` that makes the same random choices in the same order.
- `tests/golden.rs` replays runs recorded in Python and compares every cell.

The catalogue (`assets/catalog.json`) and the test data come from the Python repo:

```sh
python tools/export_catalog.py ../opensnake-rs/assets
```

## Controls

- Arrow keys or WASD to steer. On touch screens, swipe.
- P or Space to pause, R to restart, Esc to go back.
- 1, 2, 3 and 4 switch tabs.

Licensed under the BSD 3-Clause License (see LICENSE).
