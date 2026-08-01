# The README clip rig

Records a whole CIVVIS spectator game to a video, with a scripted camera over
the top of it. Built 2026-07-28 for PR #528 (the README's full-game science
victory), and kept here because the operator wants the clip refreshed with
better takes over the following days.

Nothing here belongs to the repo — it drives a *copy* of the shipped binary and
`web/` in a scratchpad, and never touches the live supervisor on 8766.

## The run, end to end

```sh
mkdir -p rig && cd rig
cp /Users/martin/civvis-spectator-src/target/release/civvis .
cp -R /Users/martin/civvis-spectator-src/{web,data} .
cp ~/civvis-readme-rig/* .

# 1. find a seed whose game ends the way you want. Through the SERVER — see below.
python3 seedsearch.py 2001 10 5          # first seed, count, parallel
python3 watch.py 9201 9202 9203          # poll live games every 2s so no result is missed

# 2. write down when everything happened in that game
python3 timeline.py 2004 9400            # → timeline-2004.json

# 3. turn the timeline into a shot list
python3 makeplan.py timeline-2004.json plan-2004.json

# 4. record it (~20 min of wall clock for a 282-turn game)
./serve.sh 2004                          # prints "PORT PID", verified to be ours
node record.js --port <PORT> --seed 2004 --plan plan-2004.json \
     --out take1 --view balanced --width 1600 --height 900

# 5. encode
python3 encode.py take1 out1
```

## The four things that are not obvious

**A seed must be searched through the server, never `civvis simulate`.** They do
not play the same game for the same seed: simulate takes a fixed civ roster off
the top of the data file, `play` draws one. `POST /new` does reproduce a
CLI-started game exactly. And poll a searching server every 2 s, not 20 — a
finished spectator holds its result for ten seconds and then starts the next
game itself, so a slow poll reads the *following* game's turn 14 and the outcome
is gone.

**`document.hidden` flips to true part-way through a long take and Chrome stops
issuing animation frames outright.** The page's own rAF counter freezes, the
screencast goes silent, restarting the cast does not wake it — and
`Runtime.evaluate` keeps answering, so every probe still reads healthy.
`Emulation.setFocusEmulationEnabled` + `Page.setWebLifecycleState({state:
"active"})` right after boot are what make a recording longer than a few seconds
possible at all. They also make evaluates slow, which is why every camera move
is *started* by an evaluate that returns immediately and then polled through
`__dir.busy`, never awaited inside one long call.

**The sky bar is only up while home is on the stage with room around it**, so a
flight has to be set up by pulling back off the board first (`earth:90`), and
every sky visit ends by flying home. And once anybody has a satellite, any view
with the world under `SKY_SURFACE` (96 px drawn radius) animates continuously at
~30 fps — so the camera cannot be *parked* in one: forty turns of voyage at ten
seconds a turn is six minutes of real-time footage in a three-minute clip.

**One duration cap cannot serve the whole take.** Set by the camera work, the
game runs for six minutes; set by the game, the camera work plays at three times
speed. `encode.py` reads the recorder's own `events.jsonl`, keeps every picture
inside a scripted move at its real timing, and takes every sixth one outside a
move at a fixed short beat.

## What shipped

Seed 2004: Australia, Ethiopia, Gaul, Egypt, Babylon, Norway on `grand_canals_2`
/ planet / Small / Online, `max_turns` 500. Babylon takes the science victory on
**turn 282** — satellite 189, Moon 199, Mars 213, expedition launched 242 — with
Egypt and Norway's expeditions still on the road behind it.

14,507 frames over 20.6 min → 4,701 distinct → a 158 s mp4 at 1600×900 (16.3 MB)
and a 12.5 s / 900 px GIF poster (5.6 MB).

## Ideas for the next take

- The box was carrying another agent's `ai_eval --pairs 120` throughout, so the
  choreography captured at ~6 fps. Record when the box is quieter, or at
  1280×720, and the camera work stops looking like stop motion.
- A second camera pass over the *terrestrial* game — a war, a city screen, the
  tech tree — would show more of what the engine actually does.
- The expedition launch flight (a 3 s rocket) was missed: the page runs a few
  turns behind the server under load, so a beat aimed at the server's turn
  arrives early in page time. Trigger the space beats off the *page's* turn.
