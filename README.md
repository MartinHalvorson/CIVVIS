# CIVVIS

On a quest to fix Civ 6 AI.

[Better than Deity AI](https://www.youtube.com/watch?v=HgqxiWQKCqc) that plays by all the same rules as human players.

All for fun. Everything is free, open source, and runs locally. Only ask is don't use AI against other players in multiplayer online.

Design & watch Civ 6 AI simulations yourself in CIVVIS.

Or try the AI Debugger to walk forward and backwards through time to study the AI decision-making and explore alternative paths. The sim debugger is still a WIP.

Posting progress videos on the YouTube channel [CIVVIS](https://www.youtube.com/@civvis). Against five Deity AIs it wins about 1 game in 5 right now, a bit better than the 1 in 6 an even seat would get, so I wouldn't call Deity beat yet. Video production is all automated; I just stamp the ones that go to the channel. Most videos don't have audio and are simply screen recordings of games with Civ 6 and CIVVIS side-by-side.
- [Lv 8 wins playlist](https://www.youtube.com/playlist?list=PLWcKlP8e8iy4)
- [Self-play performance demos](https://www.youtube.com/playlist?list=PLPI7k77hCAR4)

Highest Level Won by Victory Type (6p continents online unless otherwise noted):
- Lv 8 - [Science - 206](https://www.youtube.com/watch?v=Gu5cxEIGTuE), [Diplo - 173](https://www.youtube.com/watch?v=CQQM2uk4AkA), [Religion - 167](https://youtu.be/a12Fevo-0DM), [Culture - 179](https://youtu.be/uMxb-OPSKy8)
- Lv 7 - ...
- Lv 6 - [Domination (4P Pangaea) - 210](https://www.youtube.com/watch?v=DMdNOQT3NS0), [Score - capped at 180](https://www.youtube.com/watch?v=d6zFjTwR_10)

Benchmark snapshot, 2026-11-22 to 11-27: 1 CIVVIS agent + 5 Firaxis AI, 6P Continents Online (equal odds would be 1 in 6).

| Level | Won | Rate |
|---|---:|---:|
| Lv 4 Prince | 39 / 39 | 100% |
| Lv 5 King | 30 / 40 | 75% |
| Lv 6 Emperor | 32 / 56 | 57% |
| Lv 7 Immortal | 16 / 53 | 30% |
| Lv 8 Deity | 46 / 245 | 19% |

Current numbers: `python3 tools/live_status.py` and [civvis.ai/tournaments/](https://civvis.ai/tournaments/). This snapshot is `python3 tools/live_status.py --since all --tag civvis-t7- --tag civvis-t8-` as of 2026-11-27 05:17 UTC.

Quick real-time demos of CIVVIS.ai: a full match in 14 seconds from the single player POV, then a self-play game in 14 seconds that ends in a Phoenicia culture win (4K on YouTube: [single player POV](https://www.youtube.com/watch?v=9c8m87e3rCg&list=PLPI7k77hCAR4), [self-play](https://www.youtube.com/watch?v=2K2mHKYZ9Rg&list=PLPI7k77hCAR4), or click the clip):

[![CIVVIS.ai — a full Civ 6 match from the single player point of view, from a lone settler to an England culture victory on turn 154, then a four-civilization self-play game on a planet world that ends in a Phoenicia culture victory on turn 143](https://github.com/MartinHalvorson/CIVVIS/releases/download/media-exhibition/singleplayer-selfplay.avif)](https://www.youtube.com/watch?v=9c8m87e3rCg&list=PLPI7k77hCAR4)
