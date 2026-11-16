# Crustacean

A snake game written in Rust with [macroquad](https://macroquad.rs), for desktop and the browser, and a client for [OpenSnake](https://github.com/CharlieKulak/opensnake) servers.

An OpenSnake server keeps accounts, snake points, skins, ranks, world records and versus matches, and any client can play on it, a bit like Matrix. Crustacean is one client; [PySnake](https://github.com/CharlieKulak/pysnake) (Python) is another. Players on either share the same accounts, leaderboards and matches. Crustacean is the default game on https://snake.charliekulak.com.

## What's in it

- **Play** every mode (Wall, Portal, Cheese, Borderless, Twin, Yin Yang, Key, Sokoban, Poison, Dimension, Minesweeper, Statue, Light, Shield, Arrow, Magnet, Gate), mixed however you like.
  - Boards go up to 1000 × 1000; big ones follow the snake.
  - You can have as much fruit as the board holds.
- **3D view**: a fixed camera over the board, for every mode, including speedruns and versus.
- **Speedruns**:
  - the built-in categories, this week's ten weekly ones, and community categories (upvote and search them);
  - PBs, WRs and leaderboards;
  - speedrun your own settings to make a community category.
- **Online versus**: ranked or unranked, public or private, with bots, invites and lag compensation.
- **Store**: the shop, loot boxes, a player market for spare copies, and your collection.
- **Battle pass**, **daily challenges** and **ranks** (with ranked-only skins).
- **Profile**: your picture, bio, friends and messages. There's also the community leaderboard.

## Download

The [Releases](https://github.com/CharlieKulak/crustacean/releases) page has builds for Linux (`crustacean-linux-x86_64.tar.gz`), Windows (`crustacean-windows.zip`) and macOS (`crustacean-macos.tar.gz`). It also has the web build (`crustacean-web.zip`) for OpenSnake servers. Install it with the server's `web-support/install-client.sh crustacean <zip or link>`.

## Building

You need [Rust](https://rustup.rs).

```sh
cargo run --release                 # play on desktop
cargo test --release                # checks the rules match the OpenSnake server exactly
./web/build-web.sh                  # the browser version, in web/dist
```

On Linux, the desktop build needs the ALSA development files (`libasound2-dev` on Debian and Ubuntu). Sound plays through PulseAudio or PipeWire when they're there.

## Playing by the server's rules

An OpenSnake server checks every speedrun by replaying it with its rules, which are written in Python. So Crustacean's rules have to match them exactly:

- `src/pyrandom.rs` is CPython's random number generator.
- `src/game.rs` is a line-by-line port of the server's `game.py` that makes the same random choices in the same order.
- `tests/golden.rs` replays runs recorded by the server's rules and compares every cell.

The catalogue (`assets/catalog.json`) and the test data come from the server's repository:

```sh
python tools/export_catalog.py ../crustacean/assets
```

The server's [protocol documentation](https://github.com/CharlieKulak/opensnake/blob/main/docs/protocol.md) describes everything Crustacean talks to.

## Controls

- Arrow keys or WASD to steer. On touch screens, swipe.
- P or Space to pause, R to restart, Esc to go back.
- 1, 2, 3 and 4 switch tabs.

## License

Crustacean is free software under the GNU General Public License, version 3 or (at your option) any later version; see [LICENSE](LICENSE). Credits for the font and packs are in [CREDITS.md](CREDITS.md).
