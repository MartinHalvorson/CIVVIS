# CIVVIS

On a quest to fix Civ 6 AI.

[Better than Deity AI](https://www.youtube.com/watch?v=HgqxiWQKCqc) that plays by all the same rules as human players.

**CIVVIS is now playable.** Play against it in your own copy of Civ 6: you take the first seat of a Hot Seat game and CIVVIS plays the others. [Setup is four lines in Terminal](#play-against-civvis-in-your-own-civ-6).

All for fun. Everything is free, open source, and runs locally. Only ask is don't use AI against other players in multiplayer online.

Design & watch Civ 6 AI simulations yourself in CIVVIS.

Or try the AI Debugger to walk forward and backwards through time to study the AI decision-making and explore alternative paths. The sim debugger is still a WIP.

## Play against CIVVIS in your own Civ 6

Works today on a Mac with Apple silicon and the Steam version of Civ 6 with Gathering Storm. Windows isn't tested with the game yet. You need Apple's Command Line Tools (`xcode-select --install`, for git and Python 3) and [Rust](https://rustup.rs).

1. In Terminal:

   ```bash
   git clone --depth 1 https://github.com/MartinHalvorson/CIVVIS
   cd CIVVIS
   cargo build --release --locked --bin civvis_orders
   python3 tools/civ6_hotseat.py --manual-lobby
   ```

   The build takes several minutes. The last line installs the CIVVIS mod into the game and waits. The first time, macOS may stop it with an App Management prompt or "Operation not permitted": allow Terminal under System Settings → Privacy & Security → App Management, then run that line again.
2. Start Civ 6. If it was already open, quit it and open it again.
3. Multiplayer → Hot Seat. Pick any map and settings, then Confirm Settings.
4. The first row is you. Add a player for each seat CIVVIS should play (up to five) and set it to Human. Rows left as AI are the game's own AI.
5. Start Game.

Your turns play like single player, and a cover hides the screen while CIVVIS plays its turns. Keep the Terminal window open: Ctrl-C there stops CIVVIS and leaves your game alone. To remove the mod, quit Civ 6 and run `python3 tools/civvis_hotseat_app.py uninstall`.

Installing resets your Civ 6 mod groups to their defaults, and a UI mod of yours that replaces one of the same screens won't load while CIVVIS is installed. Everything else is in [the full guide](docs/CIV6_HOTSEAT_RELEASE.md), which is written for a packaged download that isn't released yet: where it says `civvis-hotseat`, type `python3 tools/civvis_hotseat_app.py`.

## Progress

Posting progress videos on the YouTube channel [CIVVIS](https://www.youtube.com/@civvis). Against five Deity AIs it wins about 1 game in 6 right now, which is what an even seat would get, so I wouldn't call Deity beat yet. Video production is all automated; I just stamp the ones that go to the channel. Most videos don't have audio and are simply screen recordings of games with Civ 6 and CIVVIS side-by-side.
- [Lv 8 wins playlist](https://www.youtube.com/playlist?list=PLWcKlP8e8iy4)
- [Self-play performance demos](https://www.youtube.com/playlist?list=PLPI7k77hCAR4)

Highest Level Won by Victory Type (6p continents online unless otherwise noted):
- Lv 8 - [Science - 206](https://www.youtube.com/watch?v=Gu5cxEIGTuE), [Diplo - 173](https://www.youtube.com/watch?v=CQQM2uk4AkA), [Religion - 167](https://youtu.be/a12Fevo-0DM), [Culture - 179](https://youtu.be/uMxb-OPSKy8)
- Lv 7 - ...
- Lv 6 - [Domination (4P Pangaea) - 210](https://www.youtube.com/watch?v=DMdNOQT3NS0), [Score - capped at 180](https://www.youtube.com/watch?v=d6zFjTwR_10)

Benchmark snapshot as of 2026-11-28 02:22 UTC, games won / played: 1 CIVVIS agent + 5 Firaxis AI, 6P Continents Online (equal odds would be 1 in 6).

| Level | Last 24 h | Since 11-22 | Rate since 11-22 |
|---|---:|---:|---:|
| Lv 4 Prince | 2 / 2 | 41 / 41 | 100% |
| Lv 5 King | 1 / 1 | 31 / 41 | 76% |
| Lv 6 Emperor | 2 / 2 | 34 / 58 | 59% |
| Lv 7 Immortal | 0 / 1 | 16 / 54 | 30% |
| Lv 8 Deity | 15 / 106 | 59 / 340 | 17% |

The last 24 h of Deity games, by which victories the game allowed:
- Diplomatic off (Tournament 8, which finished at 21 / 100): 13 / 53, every win Religious.
- Diplomatic and Religious both off (a new benchmark table since 11-27): 1 / 46, a Culture win.
- The benchmark's other Deity games: 1 / 7, a Diplomatic win.

Current numbers: `python3 tools/live_status.py` and [civvis.ai/tournaments/](https://civvis.ai/tournaments/). "Since 11-22" is `python3 tools/live_status.py --since all --tag civvis-t7- --tag civvis-t8-`, and "Last 24 h" is the same count over the games that ended in the 24 hours before the snapshot.

Quick real-time demos of CIVVIS.ai: a full match in 14 seconds from the single player POV, then a self-play game in 14 seconds that ends in a Phoenicia culture win (4K on YouTube: [single player POV](https://www.youtube.com/watch?v=9c8m87e3rCg&list=PLPI7k77hCAR4), [self-play](https://www.youtube.com/watch?v=2K2mHKYZ9Rg&list=PLPI7k77hCAR4), or click the clip):

[![CIVVIS.ai — a full Civ 6 match from the single player point of view, from a lone settler to an England culture victory on turn 154, then a four-civilization self-play game on a planet world that ends in a Phoenicia culture victory on turn 143](https://github.com/MartinHalvorson/CIVVIS/releases/download/media-exhibition/singleplayer-selfplay.avif)](https://www.youtube.com/watch?v=9c8m87e3rCg&list=PLPI7k77hCAR4)

## Licence and trademarks

CIVVIS's own code and docs are under the [MIT License](LICENSE). Work by others that the repository carries or its programs compile in keeps its own licence, listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md): Stockfish in Wizard Chess is GPL-3.0, for one.

CIVVIS is a fan project, not affiliated with or endorsed by Firaxis Games, 2K or Take-Two Interactive. Sid Meier's Civilization and Civilization VI are their trademarks. You need your own copy of the game to play against it.
