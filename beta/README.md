# civvis.ai — the published build

CIVVIS runs as a desktop program: a Rust engine with a local HTTP server, and
`web/index.html` talking to it over `fetch`. This directory publishes that same
program to the web with **nothing running behind it** — the engine is compiled
to WebAssembly and the page answers its own requests.

That is what makes the hosting free and what makes it honest. There is no
server to pay for, no game state anywhere but the visitor's own tab, and the
viewer is not a reimplementation: it is `web/index.html`, copied byte for byte
apart from the asset paths a subdirectory forces.

```
civvis.ai                             the STABLE lane — the promoted build, moved by a person
civvis.ai/home                        its landing page
civvis.ai/charts/ai-insights          AI Insights, the page Charts/ opens: what the games show about AI at large, an insight at a time; first, the games against the AI ending sooner at each level from Lv 5 to Lv 8
civvis.ai/charts/civ-ai-interpretability  Civ AI Interpretability: one turn step by step, the plan through a game, every gene by what it is worth
civvis.ai/charts/civ-analytics        Civ Analytics: a game's decisions in the order it asks them, each with CIVVIS's win rate by what it did: early build orders, 2nd and 3rd cities and cities at turn 50, religion, invasion, science at turn 100, victory aimed for; then the winningest civs
civvis.ai/charts/win-analytics        Win Analytics: who wins, how and when
civvis.ai/charts/elo-rankings         ELO Rankings: the agents and the Firaxis AI at Lv 4-8 on one Elo scale, now and over time
civvis.ai/charts/civvis-performance   CIVVIS Performance: the fastest game on video and how long a self-play game takes
civvis.ai/tournaments                 Tournaments: series of verification games, running, scheduled and finished
civvis.ai/download                    its native-binaries page, from the latest GitHub release
civvis.ai/test                        the HEAD lane — latest main, republished automatically
civvis.ai/test/home                   head's landing page
civvis.ai/test/charts/...             head's pages of Charts/, by the same six names
civvis.ai/test/tournaments            head's Tournaments page
civvis.ai/test/download               head's binaries page
```

**An address follows the page's name in the site list** (operator 2026-11-23:
"clean up the URL please. standardized. please remove research from the
url"). It is the name as the list shows it, in lower case with a hyphen for
anything that is not a letter or a digit, and a page listed under a folder
(Charts/) sits in that folder: Win Analytics is `/charts/win-analytics/`,
Tournaments is `/tournaments/`. `/charts/` itself has no page and opens
the first page of the folder, as Charts/ does in the list. A page added to
the list takes its address by the same rule, and a page renamed in the list
moves with its name; `beta/addresses_test.py` holds every page and every link
to it.

A page can move more than once, and `MOVED` is a table of single moves: AI
Interpretability went to `/charts/ai-interpretability/` that day and to
`/charts/civ-ai-interpretability/` on 2026-11-24, when it was named Civ AI
Interpretability (operator: "please change AI interpretability to "Civ AI
Interpretability""). An address a page has left is followed move by move to
the first one the lane has the page at, so `/interpretability` reaches the
page in a lane from before the new name and in one from after.

Until that day four pages answered where their names were not: Win Analytics
at `/research` (what the page was called the day it was written), CIVVIS
Performance at `/performance` and AI Interpretability at `/interpretability`
(at the top of the site, though the list keeps them under Charts/), and
Auto-Tourneys at `/tournaments`. Those addresses are in links this site does
not control, so each **forwards** to its page (`MOVED` in `_worker.js`, a 301
with the query kept), in a lane that has the page at its new address.

On 2026-11-24 Auto-Tourneys took its first name back, Tournaments (operator:
"change auto-outnrye to Tournaments. Auto-tourney suggests some self
imrpvoing tourney thing"), and with it `/tournaments`, so `/auto-tourneys`
forwards there now. A lane pinned to a commit from the day in between has
the page at `/auto-tourneys/`, and there `/tournaments/` forwards to it
(`BETWEEN` in `_worker.js`, `verify.BETWEEN`) until a promotion moves it.

**Where a page is served is the lane's own commit's to say**, like the page
itself. Every link in a page is relative, so a page works at one depth only,
and `publish.sh`, which is `main`'s, lays out both lanes. So each page states
its address (its `<link rel="canonical">`), and `publish.sh` puts it there; a
page from before pages said so is put where it was written for. A lane
pinned to a commit from before the move therefore keeps the old layout, and
there `/research/` is still the page and `/charts/...` a 404, until the move
is promoted. The source files kept their names (`beta/research.html` is Win
Analytics, `beta/tournaments.html` Tournaments), as did the data slots and
`data/research/`: nothing a reader's address bar shows.

**The site is two whole sets of pages, from two commits: one test set of
everything and one prod set of everything.** Each lane is a
complete tree — the viewer at its root, `/home`, the pages of `/charts`, `/tournaments` and `/download` beside it —
and every link inside a lane is lane-relative, so reviewing `/test` means
clicking around `/test` and never landing in prod. `/test` follows `main` on
a schedule so the newest site is always reviewable without anyone deciding
anything; `/` is whatever the `site-stable` tag names and moves only when the
tag does: when somebody runs the **to-prod-manual-only** workflow, and, since
2026-11-23 and for now, every six hours when **to-prod-auto-6h** puts the tag
on the commit `/test` is serving. Each lane states
its commit and its pages in its own `build.json`. Both lanes ship in every deployment —
Pages deployments are immutable snapshots of a whole directory, so there is no
such thing as updating half a site; a promotion is nothing but moving the tag
and deploying again, which promotes every page at once.

**A lane is one commit's site and nothing else.** Everything a visitor is
served in a lane is read from the lane's own commit: the engine, the viewer
and its scripts (`shim.js` and `worker.js` with them), every page, the script
that fills a page's figures (`beta/*_page.py`, `home_rankings.py`) and the
data that script reads (the snapshots and definitions under `data/`, the
Elo, the gene ranking). So nothing reaches `/` except by a promotion:

- A page that only `main` has answers at `/test/<page>/` and is a **404** at
  `/<page>/` until the commit that added it is promoted. So does an address
  only `main` has: a page that moves is at its new address on `/test` at
  once and on `/` when the move is promoted, with the old one forwarding from
  then.
- A refreshed snapshot, a newly scheduled tournament, a new record video or a
  re-rated Elo is on `/test` once it is on `main`, and on `/` once promoted.
- A lane built from a commit that is not the invoking tree's own (the stable
  lane, on every deployment) is filled by that commit's scripts. If one of
  them fails, its page is published without figures and the build carries a
  warning; the same failure in `main`'s own scripts stops the publish.

The files in a deployment are not the last word on what an address answers.
**Pages goes on serving a file it has served before for up to a week after a
deployment removes it**, from each data center's cache (its "asset
retention"), so a page that leaves a lane would go on answering there for
every reader near a data center that had served it. The AI Interpretability
page did on 2026-11-23: a 404 on `civvis.pages.dev` and with a query string,
and the old copy at `civvis.ai/interpretability/` itself. So `_worker.js`
holds each page address to the lane's own list (`pages` in its `build.json`)
and answers 404 for a page the lane does not carry, whatever the asset layer
kept. When checking that a page is gone from a lane, ask the address as a
reader would, not only `pages.dev`.

Two things in a deployment are not a lane's, and they are the whole list:

- **The serving shell**, `_worker.js` and `404.html`. A deployment has one
  request handler and one 404 page, so they are `main`'s for both lanes, and
  a change to them reaches `/` without a promotion. The 404 page offers the
  pages of the lane the missed address was in; the report dialog posts to its
  own lane (`/report`, `/test/report`), so an issue names the build it was
  filed from.
- **The run ledger**, `results.jsonl` on the `ledger` branch. The games
  played are not a property of any commit, so each lane counts them as of the
  build, with its own script.

Until 2026-11-22 a lane took `shim.js`, `worker.js`, the filling scripts,
most of their data and any page its commit lacked from `main`. The front page
therefore moved with every merge, and the AI Interpretability page answered
there the day it was written, unpromoted. `beta/publish_lanes_test.py`
publishes lanes from a small repository of an earlier and a later commit
and holds each of the rules above, the two layouts among them;
`beta/verify.py` holds every deployed lane to the pages its `build.json`
lists, at the file and at the address, and every address a page has left to
forwarding where the lane has the page.

Repeat visitors cannot be left on an earlier deployment. Each lane's HTML is a
moving pointer and revalidates on every visit. The publisher gives `shim.js`,
`worker.js`, `civvis.wasm`, and every referenced atlas a query version derived
from that file's bytes. A changed file therefore has a new URL and is fetched
after the fresh page arrives; an unchanged file keeps its URL and can be reused
from cache. Legacy unversioned URLs also revalidate. An already-open tab keeps
running its loaded build until it is reloaded.

(`/rust` and `/wasm`, the old build-channel redirects, were dropped in favour
of the real addresses above; they 404 like any other retired URL. The
installed desktop apps' local `/wasm/` channel is unrelated and remains.)

`/` serves the product directly. Setting `ROOT_REDIRECT` in the Pages
environment turns the root into a **302** to anywhere — the escape hatch that
once pointed the domain at the YouTube channel — and unsetting it (or `off`)
serves the site again; neither needs a deploy. The redirect is deliberately
temporary: browsers cache a 301 effectively for ever, and this pointer exists
to be movable.

## How it fits together

| Piece | What it does |
| --- | --- |
| `src/wasm.rs` | The engine's request router for the browser. A child module of `server`, `cfg`-gated to wasm, answering the same endpoints over the same JSON. |
| `beta/worker.js` | Runs the module off the main thread. A turn is not a quick call, and the viewer paints on `requestAnimationFrame`; on the page's own thread the engine would stall the frames it exists to produce. |
| `beta/shim.js` | Intercepts `fetch` before it reaches the network. Also owns the three things that genuinely became the page's job: the turn clock, the selected between-game finale countdown, and saved games in IndexedDB. |
| `beta/_worker.js` | The whole site's routing: the two lanes, a 404 for a page its lane does not list, a forward from an address a page has left to where the lane has it (`MOVED`) and from a folder to its first page, the optional gates on `/` and `/test`, and the response headers. |
| `beta/landing.html` | `civvis.ai/home` (and `/test/home` from head's revision) — the landing page, one of the site's five pages: the list of all five beside its title (Home, in gold because it is this page; Charts/; Tournaments; CIVVIS, the simulation the AIs play; and AI Debugger, the same game with a seat of your own through `?mode=play`) with a Civ 6 link to the game's shop page under them, the project's links (GitHub and YouTube in their own colours, GitHub saying "Everything open source, free", and under YouTube the channel's two top playlists, Lv 8 Deity Wins and Lv 1 - 7 Archive, one to a line; the same on every page), one paragraph, the Firaxis Civ 6 AI benchmark as Win Analytics has it (the two tables of win rates by level, with the Diplomatic victory allowed and with it off; the same record, written into the page's `home-benchmark` slot by `home_rankings.py` at publish, and markup, styles and script that `home_rankings_test.py` holds to that page's) and under it six videos, two by two by two: the fastest recorded CIVVIS self-play game upper left (whichever record leads `data/performance/fastest_game_videos.json`, written into the page's `home-videos` slot by `home_rankings.py` at publish, with the game it played, its `setup`, as the tile's second line), a single player's view of a game upper right (from 6 seconds in, its `data-start`), two Lv 8 Deity wins under them (France by science at the left, Maya by culture at the right) and under those the AI Debugger and Doctor Strange's Civ 6. A tile is the video's title and under it the video's picture, which gives its place to the YouTube player, playing without sound, once the tile is on screen (a visitor who asked for reduced motion keeps the pictures and presses one to play it). Every video on the site loops for as long as the page is open (`loop=1` with the video as its own playlist); YouTube's loop comes round to 0 s, so the single player's view is sent back to its 6 seconds each time round through the player's messages (`enablejsapi=1`). The two rankings that stood here moved to Win Analytics on 2026-11-22. The viewer's masthead carries the same six in the same order down its left side, with whichever of CIVVIS and AI Debugger it is showing in gold. The list is a tree that keeps its branches folded: a page's own pages are listed under it only while it is the page on screen. Charts has one, Performance (`beta/performance.html`), which Charts lists under itself and which lists itself there, in gold; the viewer (CIVVIS and AI Debugger, the two tools) lists no branch. |
| `beta/research.html` | `civvis.ai/charts/win-analytics` (and `/test/charts/win-analytics`) — Win Analytics, the fourth page under Charts/ (the first, and the one Charts/ opened, until 2026-11-24; the page was named Research, then Musings, then Charts, until 2026-11-22: these file names did not move, and the address was `/research` until 2026-11-23). Charts/ stands between Home and Tournaments in every page's list, with its six pages, AI Insights, Civ AI Interpretability, Civ Analytics, Win Analytics, ELO Rankings and CIVVIS Performance (in that order since 2026-11-24, operator: "move ai interpretablity to the top of the charts/ then civ analytics, win analytics. elo rankings.. civvis performanc"), under it on those pages: charts read off the games the CIVVIS AI agents play. The page opens on the benchmark against the game's AI, across the page (operator 2026-11-23: "our primary Firaxis Civ 6 AI benchmark - win rate on each level"): two tables side by side over the finished games of the tournaments the Tournaments page lists, the games that allowed the Diplomatic victory at the left and the games played with it off at the right (the other way round until later on 2026-11-23: "put diplo victory allowed to the left of no diplo victory"), each a row per level from Lv 8 Deity down to Lv 4 Prince and a column for the last 1, 3, 7 and 30 days, a cell reading the win rate and its games, `14% (181)`. Under it, across the page, are the graphs, CIVVIS self-play's first and the games against Civilization VI's own AI under it, each game length split by victory type. The two rankings that stood to their right from 2026-11-22 (the home page's until then) and the tree of build orders that stood over the benchmark on 2026-11-23 are on ELO Rankings and Civ Analytics since that evening. (Until later on 2026-11-22 the page was two equal columns, the AI's games on the left and self-play on the right.) The first is when each victory type lands, by Online-speed turn, in CIVVIS against Civilization VI's own AI and in CIVVIS self-play (every seat an agent, in the simulator), six players on Continents to 250 turns. A slider over the games against the AI picks the AI's level, Lv 4 Prince to Lv 8 Deity (the page opens on Deity; operator 2026-11-23: "ai level in charts can go from lv 4 to lv 8"), and the line of counts under their name gives CIVVIS's win rate at it. The games against the AI are three charts of stacked columns per span of turns (every victory, the AI's wins, CIVVIS's wins) and self-play is one; then a small chart per victory with both kinds as lines, and the figures as tables. Each kind of game counts a window of its own, last 7 days, last 30 days or every game: self-play opens on 7 days and the games against the AI on 30; 5- or 10-turn spans. Self-play's control also offers its last 1 day and last 3 days (operator 2026-11-24: "to win analytics civvis self play, add 3 days and 1 day options please. 1 day covers last 24 hrs"): the 24 and the 72 hours before its newest game finished, counted game by game when the snapshot is taken (`research_page.py --self-play` reads each game's finish off its part of the batch: the part's last write, less the lengths of the games after it) and kept under `last` in `data/research/self_play_endings.json`; the line under the title says the hours and the moment they run back from. |
| `beta/research_page.py` | That page's figures. `publish.sh` runs it and writes the result into the page's `research-data` slot. Self-play comes from the snapshot `data/research/self_play_endings.json` (the lane's own commit's), which `research_page.py --self-play <state dir>/batches data/research/self_play_endings.json` rebuilds from the gene-screen batch rows on the machine that plays them; the Deity games come from `results.jsonl` on the `ledger` branch, each kept only when its run summary there says Online speed and a limit of at least 250 turns. The benchmark's two tables of win rates (`tournament_rates`: the games played with the Diplomatic victory off, and the games that allowed it, each by level over the last 1, 3, 7 and 30 days) are counted off the same result lines by the tournaments' definitions, `data/tournaments.json`, through `home_rankings.tournament_series`, so they hold the games the Tournaments page holds; which table a game stands in is read off its run summary on the ledger. The tree of build orders (`build_orders`), which Civ Analytics shows and Win Analytics is not sent (`for_page`), counts every finished game against Deity on the ledger with one CIVVIS agent in it. A game's opening is the capital's build order, as `civ6_ladder.opening_builds` reads it off the game's events: what its production finished and, marked `:dropped`, what it put production into and dropped unfinished (operator 2026-11-24: "capital cant make a settler first - needs 2 pop": a Scout taken up on turn 1 and dropped at pop 2 is "Scout, dropped", then "Settler"), every two boards in a row compared, within a turn too (a chop finishes an item mid-turn), an item done only when the progress toward it is gone, so a bought or handed-over item (a pantheon's free Settler) is not one. The harness writes it into the run's summary (`build_order`, with `build_order_reading`, the way it was read: `civ6_ladder.BUILD_ORDER_READING`), where this script reads it when that is the way the reader reads now, and the other games are in the snapshot `data/research/build_orders.json`, which `research_page.py --build-orders . data/research/build_orders.json` fills from the event logs on the `ledger` branch (run it after `git fetch origin`; it adds the games the snapshot does not hold and whose summary carries no opening read that way, about a minute for a thousand games, and reads every game afresh when the snapshot's `reading` is not the reader's, or with `--again`). A publish reads off their logs the few openings neither holds, at most `BUILD_READ_AT_PUBLISH`. What it cannot read costs the page its figures, never the publish. |
| `beta/elo.html` | `civvis.ai/charts/elo-rankings` (and `/test/charts/elo-rankings`) — ELO Rankings, the fifth page under Charts/, after Win Analytics (operator 2026-11-23: "add that to a new page: Charts/ELO Rankings. this should be after win analytics in the ordering. at the top of this page is the player straetgies, pulled from win analytics"). At the top, the player strategies: the Civ AI Agents by Elo, with Civilization VI's own AI at Lv 4 to Lv 8 among them as the benchmark measures it; beside them, each level's two readings, the benchmark's games against the real AI and the simulator's Difficulty dial (our stand-in agent with the level's handicap). Under them the Elo over time (operator: "a running graph over time of the top 10 elo civvis ai agents, as well as our elo estimations benchmarks for the lv 4 - 8 firaxis civ ai"): each agent's rating at every published fit, a step a fit, and each Firaxis level a point a day, every line named at its end, a crosshair listing every line's value, and every point as a table. |
| `beta/elo_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `elo-data` slot. The agents are the lane's own `data/strategy_elo.json` and every fit before it, `data/strategy_elo_history.json` (`tools/strategy_elo_history.py`; the rating run appends its point). The Firaxis levels are rated from the benchmark (operator 2026-11-23: "we should have our benchmarking games update our elo"): every finished tournament game at Lv 4 to 8 on `results.jsonl`, through `home_rankings.tournament_series`, each a table of our seat and five Firaxis AIs, our seat rated as the agent that plays its lane in the simulator (`TWINS`) as published when the game began; the fit is `strategy_elo`'s model, solved for each level (`fit_level`), over the last 7 days for its rating now and for each day's point. So the levels move with every finished game, whatever the rating runs do. See `docs/ELO.md`. What it cannot read costs the page its figures, never the publish. |
| `beta/civs.html` | `civvis.ai/charts/civ-analytics` (and `/test/charts/civ-analytics`) — Civ Analytics, a page under Charts/ (operator 2026-11-23: "add another page: Civ Analytics"). One timeline of a game's decisions (operator 2026-11-24: "reflect on the top most important decisions a civ player has to make throughout the game. rank them into a list chronologically ... so the whole thing is really one logic timeline of making decisions in civ"): under the intro, an index of six decisions in the order a game asks them, each with the turns it is open at Online speed as a bar on the game's 250 and a link to its charts (1 what the capital builds first, turns 1–30; 2 how fast and how wide to expand, 1–50; 3 a pantheon and a religion or none, 10–60; 4 holding the land when a neighbour invades, 30–150; 5 keeping up with the leader's science, 50–150; 6 which victory to race for, 100–250), and the charts below stand in that order, each decision headed by its place, its turns and why it matters. The civ is set before turn 1 by the game's setup, not by CIVVIS, so its record stays at the bottom. From the top (operator 2026-11-24: "top chart is build order", "move winngest civs to the bottom"): the tree of build orders (operator 2026-11-23: "add a tree for the early build order win rates for deity games ... sort the tree by win rates descending"): for the games against Lv 8 Deity AI, the capital's first step, best win rate first, each row a step with the win rate and its games, `5.1% (1,174)`; pressing a step lists the steps after it, the same way, and so on eight steps deep. A step is what the capital's production finished, or, marked, what it put production into and dropped unfinished (operator 2026-11-24: "capital cant make a settler first - needs 2 pop": a Scout taken up on turn 1 and dropped at pop 2 is "Scout, dropped", then "Settler"). Then, under one row of filters (games: Lv 8 Deity first picked, down to Lv 4, or CIVVIS self-play; the Diplomatic victory any, on or off; the last 7 days, 30, or all), the win rate by the turn the second city was founded and by the turn of the third, a column per span of turns (operator 2026-11-23: "a 2nd city down vs win rate graph by lv default lv 8 and 3rd city down turn win rate"), and by the cities CIVVIS held at turn 50 (beside it, its median cities against the best rival it had met and the share of games level with that rival or ahead), the win rate with a religion founded and with none, and a list of win rates per pantheon belief, per founder belief and per follower belief (operator: "filterable by the typical stuff (lvs, etc). we should have a win rate for each pantheon belief and a win rate for each religion first belief"), and the win rate by the turn another civ first invaded CIVVIS, a column per span of turns and one for never (operator 2026-11-24: "make a chart of the win odds by the turn that a civ is first invaded by another civ and add to civ analytics"; an invasion is a major civ's attack on one of our cities or districts, or on one of our units where it or its attacker stood on our land), the win rate by CIVVIS's science at turn 100 as a share of the best rival's (beside it, the median share and the share of games at half or more), and per victory the game's seat was set to aim for (operator 2026-11-24: "make a top chart or two that shows monitors the problem"). Last, the winningest civs: against Lv 8 Deity AI since 2026-11-16, and beside it the self-play seats, under a note that a civ's record is its own bonuses and how well the AI's strategy uses them (operator: "winningest civs is just due to whatever modifications and chosen for the civ and how well the ai straetgy handles that"). |
| `beta/civs_page.py` | That page's record, written into its `civ-data` slot: the tree as `research_page.research` builds it (`build_orders`), taken from that record so the two pages cannot read an opening two ways; a row per finished game at Lv 4 to Lv 8 with its level, Diplomatic switch, result, day, second and third city turns, pantheon and founding beliefs (`games`), off the run summaries' `founding_marks` and the snapshot `data/research/game_marks.json` (`--marks` adds the games since from the event logs), and last the turn another civ first invaded it (`invaded`: 0 for never, -1 when nothing says), off the run summaries' `first_invasion` (`civ6_ladder.invasion_reading`) and the snapshot `data/research/first_invasions.json` (`--invasions` adds the games since), and after it the game's cities and the best rival's at turn 50, its science at turn 100 as a percent of the best rival's and the victory it aimed for (`progress`, off the run summaries' `economy_marks` and `victory_target`; -1 where no summary holds a mark, 0 for no target); the self-play seats batch by batch (`self_play`), off the snapshot `data/research/self_play_seats.json` (`--self-play <batches_dir>` rebuilds it); and the civs as `home_rankings.civs` counts them. |
| `beta/performance.html` | `civvis.ai/charts/civvis-performance` (and `/test/charts/civvis-performance`; `/performance` until 2026-11-23) — CIVVIS Performance, the sixth and last page under Charts/ in the list: it stands under Charts/, set in, after AI Insights, Civ AI Interpretability, Civ Analytics, Win Analytics and ELO Rankings, on those pages and on itself and on no other page. How fast a complete game is played. It opens on its top chart, CIVVIS computer control: game speed over time (since 2026-11-24): how long one game takes with CIVVIS at the controls of Civilization VI itself, end to end, as four tiles (the last seven days' middle game and how many times faster than by hand that is, the same per turn, the newest day, the record's first week) and a line of each day's middle game inside the band that holds eight games in ten, for a whole game or for one turn. Then how many times faster than a game played by hand, as tables. Then the same figures as a line, the speed timeline (`#perf-line`, "Civ Game Speedup - Synthetic Data Pipeline"), which stood on the home page over the videos until 2026-11-24 and is drawn here in the middle of the page (`beta/speedup_timeline.py`); under it each way of playing over time, and the path through one game. Then the fastest game on video: the record's picture, which plays the YouTube video in its place when pressed (nothing of YouTube's but that picture is asked for before then), its time as the one large figure, and the earlier records under it. Then how long a self-play game takes (every seat an agent, six players on Continents, Online speed, 250 turns): a tile each for the 1st, 10th, 50th, 90th and 99th percentile of the games' wall-clock seconds, and the games as columns per span of time with the five marked; latest batch, last 7 days or every game. Then game time batch by batch, the middle game as a line inside two bands (p10 to p90, p1 to p99), with every batch's figures as a table. |
| `beta/control_times.py` | The computer control benchmark, the Performance page's top chart: every game of the verification games played to a victory, timed end to end (by the harness's own clock, `seconds` on its line of `results.jsonl`, for a run since 2026-11-24, and from the launch stamp in its tag to the stamp of its result before), as a day's three percentiles, the last seven days and the first seven. Read from the `ledger` branch when the site is published, like the home page's civ table, so a finished game is on the chart with the next publish. `docs/CONTROL_GAME_TIME.md` is the write-up; tests: `beta/control_times_test.py`. |
| `beta/performance_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `performance-data` slot (from the lane's own commit). The speed timeline is the record's `timeline` and the tables over it its `speedup`, both worked out by `speedup_timeline.py`. The computer control games' times are `control_times.py`'s, from the run ledger. **A new record video is a line in `data/performance/fastest_game_videos.json`** (its YouTube link, the seconds it shows a game played in, that time as its title words it, the day it was posted, its title): the one with the fewest seconds is the page's video. The times come from the snapshot `data/performance/self_play_times.json`, which `performance_page.py --self-play <state dir>/batches data/performance/self_play_times.json` rebuilds from the gene-screen batch rows on the machine that plays them (a game per seed, its `secs`; finished batches only). The snapshot holds the percentiles and a count of games per span of time, taken on the games themselves, not the games. The page's part on the path through one game is `data/performance/path_steps.json` (`path` reads it), and **a new reading of the path is `tools/path_steps.py --binary <release civvis> --record`** (`docs/PATH_STEPS.md`). What it cannot read costs the page its figures, never the publish. |
| `beta/insights.html` | `civvis.ai/charts/ai-insights` (and `/test/charts/ai-insights`) — AI Insights, the first page under Charts/ and the one Charts/ opens (operator 2026-11-24: "the old ai interpretability should be in the process of being renamed civ ai interpretability. we want a new ai insights"). What the Civ games show about AI at large, an insight at a time, each a card: a heading, one line, and the chart it is read off. The first (operator: "for the first visualization, we should show the civvis vs lv 8 deity ai chart, switching between lv 8, 7, 6, 5 to highlight that smarter ai picks up speed. this is the process of optimization decisions chosen to move fast. as ai gets smarter, expect the pace of progress to speed up") is "Smarter AI picks up speed": a row per level, Lv 8 Deity first, whose bar is as long as the turn half of the level's games were over by, from zero; a row is a button, and under them Win Analytics' chart "CIVVIS vs Lv 8 Deity AI", by the name it has there, "Turn Length and Victory Type Histogram of All Games" (every game that ended in a victory by the turn it ended on, stacked by victory type), for the level pressed, with that turn marked by a white line and each level's turn by a small mark over the plot. The plot moves from one level's columns to the next's over half a second (at once for a reader who asked for less motion, and never waiting on a browser that draws no frames), and Play runs the levels from Lv 5 up to Lv 8. What it counts, and that a level of Civilization VI's AI is stronger by bonuses rather than by deciding differently, is under "How it is counted", closed. `beta/insights_page_test.py` holds the page and its record. |
| `beta/insights_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `insights-data` slot. Under `speed`, the games against Civilization VI's AI at Lv 8, 7, 6 and 5 (`LEVELS`, the switch's order): each level's every counted game, CIVVIS's wins among them, its days, the count on each turn per victory type, and `median`, the turn half of them were over by. The games are Win Analytics' own, counted by its functions (`research_page.level_groups`, `research_page.window`) off `results.jsonl` on the ledger branch, so the two pages draw the same games. A publish whose games no longer have that turn falling with the level says so in its log (`not_falling`, a `::warning::`). |
| `beta/interpretability.html` | `civvis.ai/charts/civ-ai-interpretability` (and `/test/charts/civ-ai-interpretability`; `/interpretability` until 2026-11-23, `/charts/ai-interpretability` until it was named Civ AI Interpretability on 2026-11-24, operator: "please change AI interpretability to "Civ AI Interpretability"") — Civ AI Interpretability, the second page under Charts/, after AI Insights, which was put over it on 2026-11-24; it was the first, and the one Charts/ opened, earlier that day (operator: "move ai interpretablity to the top of the charts/ then civ analytics, win analytics. elo rankings.. civvis performanc"; it was the last from 2026-11-22, operator: "Add a new chart to the bottom of Charts/ called AI Interpretablity"), listed under Charts/ on its six pages and on no other. How a CIVVIS AI agent decides, read from both ends. Top down: the steps of one turn as a strip of squares in source order, sorted into five kinds by what a step's name says it does (read the board and plan, research and govern, spend, fill the city queues, order the units); then the plan through a game, as the seats of the newest finished tournament batch wrote it down: the share of seats in each posture per span of turns, the bottleneck they name for their victory, what each kind of seat aims for against what it wins by, and the middle seat's cities, science, culture and military at every 25th turn. One control picks the kind of seat (every seat, its own choice, or one of the six victories) for all of them. Bottom up: every screenable gene as a dot, by its on-less-off win rate and how sure the estimate is that it helps, with the eight best and the eight worst as tables. |
| `beta/interpretability_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `interpretability-data` slot (from the lane's own commit). The turn's steps come from `tools/codemap.py`'s reading of `take_turn_inner`, each sorted by its name (`KINDS`), with the size of the shipping Rust under `src/ai`; the genes from `GENE_HEURISTIC_RANKING.md`; the plans from the snapshot `data/interpretability/self_play_plans.json`, which `interpretability_page.py --self-play <state dir>/batches data/interpretability/self_play_plans.json` rebuilds from the newest finished gene-screen batch's rows on the machine that plays them (each seat's `victory_portfolio.trace`, `player_target`, `trajectory` and result). What it cannot read costs the page that section, never the publish. |
| `beta/tournaments.html` | `civvis.ai/tournaments` (and `/test/tournaments`) — the Tournaments page (named Auto-Tourneys and at `/auto-tourneys` from 2026-11-22 to 2026-11-24, which now forwards here), between Charts and CIVVIS in every page's list. A tournament is one idea played out: a set number of verification games under fixed conditions. The page shows the one running (record, a square per game in the order played, wins by victory, the latest games), the ones scheduled after it, and the finished ones in the table that stood on the home page until 2026-11-21. Each tournament's title line is its number and its short description, "Tournament 4: 50 game series", the largest line it has; with the definition's name under it, that is the left third of every tournament in all three lists, and everything else (hyperparameters, record, squares, dates, durations, conditions, notes, result video) is on the right two thirds. The hyperparameters come first there (operator 2026-11-23: "please spell out the tourney hyperparameters for each tourney clearly"): the settings every game of the tournament was played under, a name and its value to a line, the same thirteen names in the same order for every tournament. |
| `beta/tournaments_page.py` | That page's record. `publish.sh` runs it and writes the result into the page's `tournaments-data` slot. It reads the definitions in `data/tournaments.json` (the lane's own commit's, so a tournament scheduled on `main` is listed on `/` once promoted) and counts the games off `results.jsonl` on the `ledger` branch with `home_rankings.tournament_series`, the same count the live mirror's Mode line uses. **Scheduling a tournament is adding its definition** (`name`, optional `idea`, `conditions`, `start`, `series_size`, `match`): a definition none of whose games has finished is listed as scheduled, in file order. **A result video is a line in the file's `videos`**, the tournament's number against its YouTube link: the page links it with that tournament's games (only a YouTube watch link is carried). **A tournament's short description is a line in the file's `descriptions`**, by number in the same way (or a `description` on its definition, used when the list has no line for its number): every tournament has one, so add it when a tournament is scheduled or begins. **A tournament's hyperparameters are a line in the file's `hyperparameters`**, by number in the same way (or `hyperparameters` on its definition): Games, AI level, Players, Victory types, Civ, Strategy, Map, Game speed, Turn limit, Ruleset, Played in, Game end and CIVVIS agent, put in that order (`HYPERPARAMETERS`) whatever order the file has; add them when a tournament is scheduled. A page whose slot is still `null` says it has no record. |
| `beta/home_rankings.py` | The site's two rankings, on ELO Rankings and Civ Analytics since 2026-11-23 (`elo_page.py` and `civs_page.py` take them from here; Win Analytics had them from 2026-11-22 and the home page before, and a lane pinned to either is still filled into its `rankings-data` slot), and the home page's fastest game (`videos`: the record that leads `data/performance/fastest_game_videos.json`, written into the home page's `home-videos` slot). The rankings: the winningest civs of the verification games (our AI's win rate as each civ against Civilization VI's own AI, best first, over the Lv 8 games that began on or after 2026-11-16: `CIVS_LEVEL`, `CIVS_SINCE`), on the left, and the Civ AI Agents by published Elo (`data/strategy_elo.json`, handles from `strategies::ROSTER`) on the right. `publish.sh` runs it against the lane's own revision and writes the result into the page's `rankings-data` slot, so nothing is committed as a copy; a page whose slot is still `null` hides the lists. The civs are the one list that is not a property of the revision: they are read from `results.jsonl` on the `ledger` branch, where `tools/civ6_ladder.py publish-run` adds a line as each game finishes, so both lanes show the record as of the build (the publish workflow's checkout fetches every branch; a clone without the branch publishes without that list). `home_rankings_test.py` holds the parse, the count and the slot. |
| `beta/speedup_timeline.py` | The speed timeline of CIVVIS Performance, in the middle of that page (the home page's, over its videos, until 2026-11-24): five ways one game of Civilization VI gets played, as the project owner named them on 2026-11-24 (Human Player (me), CIVVIS → Firaxis computer control, Firaxis self-play, CIVVIS self-play, and Parallel games on every core of an M5 Max), the time each takes and how many times faster than by hand that is, on a line of multipliers by tenfolds that runs out to 10,000,000x (the ledger's `line_to`), a tenfold past the one goal, 1,000,000x. A reading that was worked out and not timed (`worked`) is marked as such, and a stop with `line` false is kept for the tables over the line alone. With them stand the stops nothing has played yet (the ledger's `projected`: the irreducible game at ten times CIVVIS self-play and the most an M5 Max could finish at ten times that, both guesses; a reasonable local hardware scale-up at ten times the M5 Max, theoretical; and a distributed RL fleet at up to a thousand times and more, a potential), each another stop's multiplier times the project owner's figure and never the page's number. Left of CIVVIS self-play the games play many at once and a time is amortized (the ledger's `amortized`): such an entry leads with its games per second, under a game a second as past it, and has its time per game as its second measure, which says amortized (`amortize`). The goal says what renting the rest of the way to it is estimated to cost (the ledger's newest `cloud` entry, which names who made the estimate; `rented` works it out, taking a rented machine to reach the share of its cores our own every-core run reached, and the Performance page shows it as a fourth table of sums). The page's one number is the fastest stop's multiplier. The readings are `data/performance/speedup_timeline.json`, dated and oldest first, the stop that plays on every core follows the record of `data/performance/games_per_minute.json`, and the computer control stop reads the record of `beta/control_times.py` where the clone holds the run ledger (`"benchmark": "control"`; its readings in the ledger are the fallback); nothing derived is committed. `performance_page.py` writes what this works out into the Performance page's slot as the record's `timeline`, and a page whose record has none shows no timeline. A new reading is a line in the ledger: `docs/SPEEDUP_TIMELINE.md` says how each stop is measured. It also works out that page's tables from the same readings (`worked`), and under them the benchmarks processor by processor (`processors`): the processors of `data/performance/processors.json` with PassMark's single thread and multithread scores for each, the one the benchmarks were run on as measured and every other as a calculation from the scores. And it builds that page's charts of each stop over time (`history`, with the `pace` each has kept): a stop's readings, the self-play tournament's batches, and the self-play benchmark hours of `beta/self_play_hours.py`. Tests: `beta/speedup_timeline_test.py`. |
| `beta/download.html` | `civvis.ai/download` (and `/test/download`). Links `releases/latest/download/<asset>`, so it never needs republishing when a release is cut. |
| `beta/chess.html`, `beta/chess/` | Not published: Wizard Chess is set aside since 2026-11-24 (operator: "lets set aside wizard chess right now. please remove from civvis.ai"). From 2026-11-23 it was at `civvis.ai/wizard-chess`, the last name in every page's list, on a chessboard of a button. No page lists it now, `publish.sh` copies nothing of it, and `/wizard-chess/` is a 404 in both lanes. The game stays here so it can come back: chess where a move is a command, said (hold Space), typed or pointed, and the piece makes it by itself and breaks what it takes, against Stockfish held to the Elo picked (`chess/`: its scripts, and three.js and the engine under `vendor/`). `docs/WIZARD_CHESS.md` says how it is built and why, and what putting it back takes. `beta/wizard_chess_test.py` still checks the rules (perft), the commands, the vendored bytes, and a game played in a browser, from this directory. |
| `.github/workflows/release.yml` | Builds those assets for Windows, macOS (both architectures) and Linux on a `v*` tag. |
| `.github/workflows/to-test-auto-30.yml` | Builds both lanes, checks them, and deploys — half-hourly behind the gate, and on demand. Without the Cloudflare secrets it degrades to a dry run, so the schedule is safe before the account exists. |
| `.github/workflows/to-prod-manual-only.yml` | Moves the `site-stable` tag to a chosen revision, then runs to-test-auto-30. One of the two ways `/` changes. |
| `.github/workflows/to-prod-auto-6h.yml` | The other: every six hours (00:12, 06:12, 12:12, 18:12 UTC) it moves the tag to the commit `/test` is serving, then runs to-test-auto-30. On for now; disabling the workflow switches it off and leaves the rest alone. |
| `beta/promotion_test.py` | Runs to-prod-auto-6h's own tag step against a small repository (the two lanes' `build.json` stood in for) and holds what it promotes and what it leaves alone, that its deployments are counted against the month, and that it and to-prod-manual-only take turns. Run on every pull request. |
| `beta/publish.sh` | Assembles one complete lane into `beta/dist/` from a named revision and from nothing else — viewer at the root, the pages that revision has beside it, each at the address it states and filled by that revision's script, every inter-page link lane-relative and landing on something the lane has. |
| `beta/addresses_test.py` | Holds every page to the address rule (the page's name in the site list), every relative link of every page to landing on a page or the viewer, and the router's table of left addresses to the publish check's and to the pages. Run on every pull request. |
| `beta/first_visit_note_test.py` | Holds every page a lane publishes, and the simulator, to carrying the first-visit note as one block that is the same in all of them, the note to the operator's words and its Ack button, and the block to what it does when run: shown once on the site, put away for good by Ack, never shown off the site or inside a frame. Run on every pull request. |
| `beta/publish_lanes_test.py` | Publishes lanes from a small two-commit repository with `publish.sh` as it ships (the engine build stood in for) and holds the lane rule: a lane's pages, scripts, figures and data are its own commit's, and only the serving shell is the invoking tree's. Run on every pull request. |
| `beta/verify.py` | Holds a lane to the pages its `build.json` lists (each present and answering at its address, every other page a 404 there), then opens the assembled bundle in a real browser, watches it play, and walks through the password door. |
| `beta/worker_test.py` | Calls `_worker.js` directly — the forward, the password, the headers — needing only Chrome. |
| `beta/debugger_test.py` | Drives the Sim debugger's bar and its split screen (`web/assets/app_debugger.js`, as it ships) against a toy engine: each control sends the operation it is for, the two halves of a head to head are told, run, compared, copied and kept, and Autosearch plays its lines on copies of the toy engine and then plays the one it found. Needs only Chrome; run on every pull request with the other `*_test.py` scripts here. |
| `beta/forks_test.py` | Forks an engine to parallel workers (`shim.js` and `worker.js`, as they ship) against a toy engine: a fork carries on from where its engine was, each plays its own history, and a request that names a fork reaches it. |
| `beta/serve.sh` | Serves `beta/dist/` locally the way a static host would. |

Nothing under `web/` is edited to suit the web build, and none of `src/wasm.rs`
compiles for a native target. The whole footprint on the desktop program is two
things in `src/server.rs`: the module declaration, and `process_identity()`.

That second one is not cosmetic. **`std::process::id()` panics outright on
`wasm32-unknown-unknown`**, and `Session::state()` called it — so every single
`/state` trapped while `/runtime` answered fine, and the page died at boot
reporting a `TypeError` reading `state.map`, having assigned it the `{error: …}`
it got back. It is the sort of failure that is invisible in a compile and
one line long in a panic message, which is why the module can now report its
own panics (`civvis_last_panic`) instead of dying as `RuntimeError: unreachable`
and a list of function indices.

### What a visitor lands on

An AI-only simulation on the stock opening world: four free-for-all majors on
the Tiny Tennis Ball planet, its heat scattered rather than banded by latitude,
with an Ancient start, Online game speed, and every victory condition enabled.
It is the world the desktop launchers open too, and the one that asks nothing
of somebody who has just arrived. `stock_opening_params` in `src/server.rs`
describes it. The site and every server open at Max Speed "Max" (unpaced; a
server opened at 2 turns/s until 2026-11-15); a cap the visitor picked by hand
(5, 2, 1, 0.5, 0.25 or 0.1 turns/s) is remembered and wins, and a Tactics
battle still opens at 2 turns/s. The lobby is right there for anyone who
wants to play a seat instead.

**The first time a person comes to the site they are shown one note**, on
whichever page that is, the simulator included (operator 2026-11-24: "please
add a note to the site when a person first visits", "with an Ack button"):
that everything is a work in progress, how the site is built (Claude from
the command line, code he neither writes nor reviews, none accepted from
others) and that all of it should be open source and free; his words are in
the block, set as three paragraphs. Ack (or Escape) puts it away, and
the browser keeps that under `civvis-first-visit-ack` for every page and both
lanes, so it is shown once. The game goes on behind it. Each page carries the
note as the same block of script at the end of its body; it shows on the site
only (the simulator's own `CIVVIS_PUBLISHED_SITE` test of the host name), so a
desktop build, a page under test and a game being filmed never show it, and
never inside a frame, so Doctor Strange's two simulators show none of their
own.

The lower-left build marker names the pinned revision and its commit time, then
ages the exact artifact in the browser. Source and packaging time stay visibly
distinct: rebuilding the same commit tomorrow produces a fresh build without
pretending Git history changed.

The world is **different every visit**. The engine is deterministic per seed and
imports nothing, so it cannot vary on its own — the page rolls a seed per load
and hands it over with the first request. `civvis.ai/test/?game=<n>` pins one,
which is how a world worth showing gets shared.

Settings travel the same way. A URL can make the lobby's choices before the
page loads — `?players=12&map=pangaea&victories=domination` opens on that
world instead of the stock exhibition, and every world after it stays on
those settings. `shim.js` reads the parameters and posts the one `/new`
request the setup screen would have; the vocabulary is the lobby's own ids:
`players`, `map`, `shape`, `poles`, `speed`, `era`, `turns`,
`victories` (a comma-separated list of tracks), `arena` (a battlefield's
dimensions, `20x20`), and `mode` (`play` seats the visitor in the first
chair for a single-player game; `watch` leaves the world to its AIs, which
is also the stock setting). A value the engine does not recognise leaves the
stock setting standing, so a mistyped link still opens on a world. The home
page's four game quadrants and its battle picker are exactly these links:
`/?mode=play` is Play Civ, `/` is Watch Civ, and a picked battle travels as
`/?map=<battle>&players=2` with `&mode=play` when it is to be played.

`/?mode=strange` is Doctor Strange's Civ 6, listed just above AI Debugger since
2026-11-23 (operator: "lest move dr strange civ 6 above ai debugger"): the
played game of `mode=play` with its two timelines drawn as a
tree up the middle of the window and the two worlds under it
(`docs/SIM_DEBUGGER.md`).

### One property worth knowing

On a socket, a spectated turn is played by a stepper thread and *delivered* to
viewers, and a great deal of machinery exists to stop a turn being simulated
before every viewer has painted the last one.

In the browser there is no stepper. A request that names the frame the page
holds **is** the step, and the viewer only issues that request after `render()`
has finished.

Left at that, a watched turn costs the engine's time plus the page's, each idle
while the other works, and in the opening turns the two are about the same
size. So at Lightning `shim.js` asks for the next frame itself, the moment it
hands the page the one before: the page's own next request, made early, so the
engine plays while the page paints. It is one frame and never two — a second is
not asked for until the page has taken the first — so the engine is never more
than one frame past the glass, and frames reach the page one at a time, in
order. The frame-per-turn contract still holds by construction rather than by
enforcement. The read-ahead is only a guess at a request: anything that changes
the world or what a state says about it (a pause, a pace, a seat to watch as, a
new game) drops it, and the engine answers what the page actually asks.
`shim_test.py` holds it to all of that against a scripted engine, and `?ahead=0`
turns it off for a side-by-side measurement.

The same file sends the viewer's opening pace and between-game hold ahead of
the page's first read, so the first state already agrees with the viewer and
nothing is posted, parsed and drawn again on the first frame; `shim_test.py`
pins those values to the viewer's own.

## Publishing

Nobody cuts test builds. `to-test-auto-30` fires **every half hour** (and on the
Actions button), and a gate decides in seconds whether a deploy is worth
twenty minutes of building: if `/test` already serves the current head it
skips, and scheduled runs pace themselves against **320 of Cloudflare's 500
deployments a month**, earned continuously rather than refilled at midnight.
A run past that pace still deploys when a change under `web/` or `beta/` has
landed since `/test` was built, up to a second line of **470 a month plus 20**,
also earned continuously. The same second line lets a run through when a
promotion is waiting: `site-stable` names a commit `/` is not serving, because
a tick took the place of the promotion's own deploy in the queue. Promotions
and manual runs are never gated. When it
does run, it rebuilds both lanes — `/test` from the head of `main`, `/` from
the `site-stable` tag — checks that each plays, and deploys them as one site.

The effect: a change to the site is on `/test` within about half an hour of
merging plus the build; a commit that touches only the engine waits for the
pace, and in a week of round-the-clock merging that spacing stretches toward
~2¼ hours instead of exhausting the month. The arithmetic that
forces a governor at all: this repository has merged several hundred commits a
day, and an ungoverned half-hour cadence is ~1,440 deployments a month against
a cap of 500.

**Promoting** by hand: when `main` is in a state worth being the
front page: **Actions → to-prod-manual-only → Run workflow**, ref = the sha you have
been watching on `/test` (or `main`). It moves the tag and runs the same
publish job the schedule runs — one code path, no drift. The gates prove a
build is not broken; whether a whole game is worth watching is the judgement
being exercised here, usually after simply watching `/test` for a while.

**Promoting on a clock** (since 2026-11-23, for now): **to-prod-auto-6h** does
the same thing every six hours with the commit `/test` is serving, so nothing
reaches `/` that was never on `/test`. It only moves forward: it leaves the
tag alone when `/` already serves that commit, when `/test` does not contain
what `/` serves or the commit somebody last put the tag on, when `/test`
serves something that is not on `main`, and when it cannot read either lane.
If its deploy fails it puts the tag back on the commit `/` is serving, so a
commit that cannot be deployed never holds both lanes; if its deploy is
cancelled the tag stays, and the next scheduled run publishes it. It spends at
most four deployments a day (~120 a month) from the same 500, none when there
is nothing to promote. **Run workflow** on it is "test to prod now". To stop
it: **Actions → to-prod-auto-6h → Disable workflow** (or `gh workflow disable
to-prod-auto-6h.yml`); manual promotion and the `/test` schedule carry on.

By hand, the equivalent of one scheduled run:

```bash
./beta/publish.sh --commit <stable-sha> --out lane-stable
./beta/publish.sh --commit <head-sha>   --out lane-test
cp -R lane-stable site                    # each lane is a whole, relative-
cp -R lane-test site/test                 # pathed site; it mounts anywhere
rm -f site/test/_worker.js site/test/404.html   # the shell stays at the root
./beta/worker_test.py                     # prove the routing behaves
./beta/verify.py --dist site --mount root
./beta/verify.py --dist site --mount test --no-gate
npx wrangler pages deploy site --project-name civvis
```

`worker_test.py` exists because `_worker.js` is the only part of the site a
static server never runs, so opening the bundle in a browser proves nothing
about it — and it is the part holding both the password and the domain's
whole purpose. `verify.py`'s `check_gate` covers the same ground against the
real runtime, but it needs `npx wrangler` and therefore Node; a machine
without Node skipped the check entirely and said so in one line nobody reads.
`worker_test.py` needs only Chrome: it imports the module and calls it, with
`env.ASSETS` stubbed to report which file *would* have been served. The one
place a browser is not the Workers runtime — `Cookie` and `Set-Cookie` are
forbidden header names on the web and get dropped — is handled in the harness,
and documented there.

Measured on this Mac, the published engine answers `/runtime` (which builds the
world) in about 120 ms, a whole `/state` document in about 95 ms, and a turn in
about 126 ms — four or five turns a second in a browser tab.

`publish.sh` refuses to assemble a page whose asset rewrites no longer match
the viewer, so a restructured `web/index.html` fails the build instead of
publishing a page with missing sprites. The check is *"no root-absolute
reference survives"* rather than *"exactly N were rewritten"*: an exact count
failed the build every time somebody drew a new atlas, which is ordinary work
and says nothing about whether the rewrite still works.

## Weight

A hosted build has to stay something a person can be handed over an ordinary
connection, and both halves of it grow every week. `publish.sh` therefore fails
above **25 MiB assembled** (`BUNDLE_BUDGET_BYTES` to move it deliberately), and
prints where the build sits against that.

Measured on `7e681b6`, which is why two things happen at publication:

| | before | after |
| --- | --- | --- |
| engine, `wasm-opt -O3` | 9,972,139 | 8,084,494 |
| atlases, lossless WebP | 12,464,506 | 8,869,674 |
| **bundle** | **24,228,282** | **18,745,813** — 71% of budget |

- **`wasm-opt -O3`** takes a fifth off the module. `-Oz` was measured against
  it and is worth 0.3% more — 8,061,627 bytes — for a module that simulates
  whole games, so `-O3` stays. Over the wire brotli makes both about 1.74 MB,
  so this is mostly about the disk figure and the budget.
- **Lossless WebP** is pixel-identical to the PNG it replaces, and the atlases
  were the heaviest thing in the bundle — heavier than the engine. It happens
  to the *copy*: `web/assets` stays PNG, which is what the desktop build serves
  and what anyone editing the art works on. Needs Pillow; without it the
  atlases publish as PNG and the only cost is the budget.

Both steps are optional and both are the first thing to reach for when the
budget bites. After them the bundle is dominated by art, so the next real lever
is the atlases themselves rather than anything this directory does.

**A revision is promotable when all of these hold** (the schedule enforces
1–5 for the test lane on its own; promotion adds the sixth):

1. It is on `origin/main` and its CI run is green.
2. `cargo test --profile ci` passes at that revision.
3. `./beta/publish.sh --commit <sha>` completes, inside the size budget.
4. `./beta/worker_test.py` reports `the site routes correctly`.
5. `./beta/verify.py` reports `this build plays` — in both lanes.
6. A whole game is worth watching — the checks above prove it runs, not that
   it is good. Watching it play on `/test` for a while is the honest test,
   and is exactly what the test lane is for.

Each lane's `build.json` records its commit and build time, so anything on
civvis.ai can always be traced back to a revision — `/build.json` for the
front page, `/test/build.json` for the head lane.

## One-time setup

### 1. The host

Cloudflare Pages, on the free plan. It is chosen over the alternatives because
it is the only free host that can run the site's routing **on the server** —
`_worker.js` is not shipped to the browser — and because it serves the module
brotli-compressed at roughly a fifth of its size.

No machine needs wrangler or Node. The whole host-side setup is two repository
secrets under **Settings → Secrets and variables → Actions**:

| Secret | Where it comes from |
| --- | --- |
| `CLOUDFLARE_API_TOKEN` | Cloudflare dashboard → My Profile → API Tokens → Create Token → **Create Custom Token**: one permission, `Account → Cloudflare Pages → Edit`, scoped to the account. (The "Edit Cloudflare Workers" template is the commonly suggested shortcut, but Pages deploys need the Pages permission, not the Workers one.) |
| `CLOUDFLARE_ACCOUNT_ID` | The right-hand column of the account's overview page in the dashboard. |

The first deploying run of `to-test-auto-30` creates the `civvis` Pages project
itself if it does not exist, then deploys into it — which gives a working URL
at `civvis.pages.dev` before the domain is attached. Without the secrets the
workflow still builds and checks both lanes and keeps the site as an artifact,
and fails with a clear message if asked to deploy — rather than appearing to
publish and doing nothing.

### 2. The domain

`civvis.ai` is registered at Namecheap, on Namecheap's own nameservers, and
currently shows a parking page.

**The nameservers have to move to Cloudflare.** This is not a preference:
Cloudflare Pages will only attach an *apex* domain that is a zone on the same
account, because putting a CNAME on a zone apex is not legal DNS and the way
round it — CNAME flattening — only exists inside Cloudflare's own resolver. An
`ALIAS` record at Namecheap (which BasicDNS does support) is enough to point
the apex somewhere, but not enough for Pages to issue the certificate and claim
the hostname. A subdomain such as `beta.civvis.ai` *could* be attached over
external DNS with a plain CNAME — that is the fallback if the move is ever
unwanted, at the cost of the URL being the one asked for.

1. In Cloudflare, **Add a site** → `civvis.ai` → Free plan. It scans the
   existing records and gives you two nameservers.
2. Delete the scanned `MX` and SPF `TXT` records — they are Namecheap's mail
   forwarding, nothing is received through them, and they will not work off
   Namecheap's nameservers anyway. See below.
3. At Namecheap: **Domain List → Manage → Nameservers → Custom DNS**, and enter
   those two. (They replace `dns1.registrar-servers.com` /
   `dns2.registrar-servers.com`.)
4. Wait for Cloudflare to report the zone active — usually minutes, and the
   registrar's own propagation can take a few hours.
5. In the Pages project → **Custom domains** → add `civvis.ai` and
   `www.civvis.ai`. The records and the certificate are created for you.

Nothing else needs a DNS record. The apex and `www` are the whole site;
`/test` and `/download` are paths on it, not hostnames, which is why there is
no `beta.civvis.ai` to set up.

### The mail on this domain

`civvis.ai` currently publishes Namecheap's free email-forwarding records:

```
MX   10 eforward1.registrar-servers.com.  (and eforward2-5)
TXT  "v=spf1 include:spf.efwd.registrar-servers.com ~all"
```

Namecheap ties that service to *their* nameservers, so moving the zone stops it
working even though Cloudflare's scan copies the records across faithfully — the
records will be right and the service behind them will not answer. **Nothing is
received at any `@civvis.ai` address** (checked with Martin, 2026-08-01), so
this costs nothing and the records can simply be dropped.

If that ever changes, the replacement is **Cloudflare Email Routing** — free,
in the same dashboard under Email, and configured in the place the DNS now
lives.

### 3. The test lane is open

`/test` asks for nothing. It was behind a shared password while it was a thing
not meant to be found; it is now the thing the channel points people at.

Setting **`TEST_PASSWORD`** in the Pages project's environment variables
(Production *and* Preview) closes it again with no deploy. There is
deliberately **no fallback password in the code**: the old one was a literal in
this public repository, which is not a password but a speed bump with a
published height. Either the environment names a secret or there is no gate.

Everything under `/test` is still sent `X-Robots-Tag: noindex`. Open to anyone
following a link is not the same as wanting an unfinished build to be the first
search result for the project's name.

The routing is a `_worker.js` rather than the more obvious Pages `functions/`
directory, and that is not a style choice. `functions/` is resolved against the
**working directory wrangler runs in**, not the directory being deployed: run
the deploy from one level up and the whole file is quietly left behind, the
upload succeeds, and the site looks like it works. That happened here once. A
`_worker.js` lives *inside* the deployed directory and cannot be separated from
it — and both checks now ask the routing rather than trusting it, including
that `TEST_PASSWORD` has not been left set by accident.

### 4. The channel

`https://www.youtube.com/@civvis`, in three places: the forward on `/`
(`CHANNEL` in `beta/_worker.js`), the landing page's first button, and the
viewer's own header link in `web/index.html`.

### 5. The downloads

`beta/download.html` links `releases/latest/download/<asset>`, which GitHub
resolves to the newest release, so the page is written once and never
republished for a release. **The asset names are therefore load-bearing and
must not acquire version numbers.** `release.yml` builds exactly those four
names; `verify.py` fails a build whose download page links an asset no release
job produces, because the alternative is finding out from a visitor's 404.

Cutting one is a tag:

```bash
git tag v0.6.1 && git push origin v0.6.1
```

The workflow's "Run workflow" button builds all four targets without
publishing anything, which is how you learn a target stopped compiling before
a tag is public.

## Limits worth knowing

- **Saved games live in IndexedDB**, as the bytes the engine produced, with what the list shows stored apart so listing never loads a game. They used to live in `localStorage`, whose few megabytes a Tiny map outgrew by about turn 130, and a save left there by an older build moves across on first use. Clearing site data still deletes them, and they stay in the browser that made them.
- **The league roster is in the bundle**, compiled in like every other file
  under `data/`, so seats carry the same ratings they do on the desktop build.
  It was read from disk until then, and there is no disk here, so every seat
  showed the provisional 1500 that means "never heard of this player". The
  public static `/test/` site remains read-only. The installed desktop `/wasm/`
  channel has a trusted local host instead: it shares the native spectator's
  live roster, records every seated AI strategy through the native Glicko
  writer, deduplicates retries in `league.json`, and supplies the updated table
  to the module before the next game.
- **Every poll carries the whole world.** The socket build sends a tile *patch*
  — about 157 KB against 1.36 MB — because it keeps a per-viewer fingerprint of
  the map. Here the page is told the world whole every turn, which is the
  full-resync path it already has. It costs serialisation, not correctness, and
  is the first thing to do if the turn rate ever needs to be higher.
- **One tab is one world.** There is no supervisor, no handoff, and no shared
  state between visitors.
- **The module is single-threaded.** The engine's parallel paths are not used;
  a turn on a very large map costs more here than on the desktop build.
