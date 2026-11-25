# Chaos Monkey

Chaos Monkey: CIVVIS worlds played under a very high frequency of natural
disasters, to learn what breaks a society, what holds, and what avoids the
worst outcomes, without living through any of it.

Operator, 2026-11-25: "A great thought today is chaos engineering society in
simulation to try to learn any lessons we can, without having to go othrough
the event ourselves to learn. above civvis add "Chaos Monkey". Here we can
run civvis sims with a very high frequency of natural disasteres. what can we
learn about avoiding horrible outcomes?"

Chaos engineering breaks a running system on purpose, on a schedule nobody
chose, to find out where it is fragile before a real failure does. Here the
system is a world of civilizations, and the breakage is the game's own
weather.

## The dial

`Game::disaster_frequency` (`src/game.rs`, read in `src/game/disasters.rs`)
multiplies how often every class of Gathering Storm's random disasters
strikes: river floods, droughts, hurricanes, tornadoes, blizzards, dust
storms, forest and rainforest fires, volcanic eruptions.

- **1** is Civilization VI as shipped at the lobby's disaster intensity. The
  lobby's five settings stop at hyperreal, 2.75 times standard. The dial goes
  on from there, to 1,000.
- It changes **how often** and nothing else. A flood at 100x is the flood the
  rules always had: the same reach, the same damage, the same Dam that stops
  it.
- At 1 the game is the game it was, bit for bit. A class's expected strikes a
  turn stay far under one in the shipped game (twelve floods a game is one
  turn in twenty), so it is one roll. Past one, the whole part strikes for
  certain and the remainder is the roll; the first strike draws from the
  stream the single roll always drew from.
- It is read on every turn's roll, so it can be turned mid-game. A save
  carries it only when it is not 1.

`chronicle.disasters` (`DisasterCensus`) counts what struck, by class, and
what each civilization lost to it: citizens, improvements and districts left
pillaged, units. Like the siege census, no rule reads it and a save does not
carry it.

## Running a world under it

In the simulator, the turn box has a **Chaos Monkey** row on a watched world:
Off, 10x, 30x, 100x, 300x, 1,000x. It turns the dial from the next turn, and the worlds
dealt after this one keep it. An address says it too:

```
https://civvis.ai/?chaos=30
```

From the command line:

```bash
civvis play --chaos 30            # a watched world at 30x
```

The row asks the engine with `POST /chaos {"frequency": 30}`
(`routes::chaos`). Only a watched CIVVIS world takes it: the Sim debugger
keeps turns ahead of the one on screen, played under the dial as it stood,
and a repainted game of Civilization VI rolls its own weather.

## The experiment

`civvis chaos-monkey` (`src/chaos_monkey.rs`) plays each seed once at each
frequency, with the stock agents on every seat, and prints one JSON record a
game as it finishes:

```bash
civvis chaos-monkey --players 6 --map continents --speed online \
  --start-seed 1 --games 24 --frequencies 1,10,30,100,300,1000 --jobs 3 > games.jsonl
python3 tools/chaos_monkey.py --write --commit "$(git rev-parse --short HEAD)" \
  --date "$(date -u +%F)" games.jsonl
```

The worlds at one seed differ in their disasters and in nothing else, so a
world under chaos has its own calm self (1x) to be read against. The agents
are not told the dial was turned.

A record holds what struck, what each civilization lost, how each stood
every ten turns (people, cities, score, technologies, army, Builders,
improved tiles and how many of them lay pillaged), the wars that started,
the cities that changed hands, and how each civilization was sheltered at
the end.

`tools/chaos_monkey.py` reads the records into `data/chaos_monkey.json`, the
record the page draws:

- per frequency: the disasters and what they cost, the world at its end as a
  share of the same seeds played calm, the horrible outcomes (civilizations
  that lost half their people or were wiped out, cities that changed hands,
  wars, nuclear strikes), how the games ended, and the world turn by turn;
- the differences between civilizations that went with keeping more of
  their people under the storm, each as two halves split at the middle
  within a frequency;
- the lessons. Each is written by a rule in `lessons()` from the record's
  own figures, and only when the rule holds. A lesson the games stop bearing
  out is not written, and `tools/test_chaos_monkey.py` fails a record whose
  lessons are not the ones its figures give.

The same seeds on the same commit give the same games, so the record names
the commit and the seeds rather than keeping the games.

## The page

`beta/chaos.html`, at `/chaos-monkey/`, between Experiments and CIVVIS in
the site list. `beta/chaos_page.py` writes the lane's own
`data/chaos_monkey.json` into it at publish. It opens on the lessons, then
the dial with what each frequency does, what gives way first, the world turn
by turn, the horrible outcomes, who comes through, and a button that opens
the simulator on a world at the chosen frequency.

## What it is not

- Not a forecast of the real world. It is one game's rules and one set of
  agents; the lessons are about how a system of competing societies with
  these rules degrades under shocks.
- The agents do not adapt to the dial. They repair and rebuild as they
  always do; nobody builds more Dams because the floods came more often.
  What a society that *plans* for the storm could do is the next experiment.
- Splits between civilizations are what went together in these games, not
  proof of what caused what.
