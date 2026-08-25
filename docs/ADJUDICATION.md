# Adjudication: measured — dead as a compute lever, shipped as a game rule

**Verdict (2026-08-07): do not adjudicate for throughput. The hypothesis that
ending "decided" games early recovers meaningful compute is refuted at every
threshold on both turn-budget regimes (tables below).**

**Product decision (2026-08-07, operator): the same threshold crossing ships
as the player-facing "Mercy Rule" setup option — a concession rule, not an
optimization.** The tables below are its calibration reference: they say what
each rung of the setup ladder trades. The related "Require N Victory Types"
setup option shares this page because both change how a game ends.

## The Mercy Rule (setup option)

The game ends the moment one civilization's live win odds — `odds::table`,
the calibrated share the spectator ribbon shows, on flat 1500 priors — reach
the chosen threshold. Setup ladder: **None (default), 99%, 97%, 95%, 90%**.
Checked at every world-turn wrap after the real victory sweeps, so mercy never
outranks a victory the rules just recognised; recorded as victory type `mercy`
and denoted as `Mercy Rule - <victory type(s)>` (see The notation below).
Every setup surface defaults to **off** (`GameOptions`, the stock opening
world and lobby, the wasm opening world, `civvis play`, and every historical
scenario), so simulation baselines and rated batch evidence are unchanged.
Select a threshold to opt in.

What the measurement says each rung trades (six-player stock budget): 0.95
crossed in 23/60 games and agreed with the played-out winner in 91.3% of
crossings; 0.98 agreed in 100% of 11 crossings; 0.90 agreed in 93.5% of 31.
A mercy game ending early may therefore occasionally crown a seat the full
game would not have — that is the rule's nature, chosen deliberately.

### The notation

A mercy ending is written and shown as **`Mercy Rule - <victory type(s)>`**,
naming the open victory lane the winner led when the odds crossed — `Mercy
Rule - Science`, or both lanes joined as `Mercy Rule - Science + Domination`
when a seat led two at exactly the same progress. The rule stops a game the
victory conditions were still deciding, so the bare word says only that it
ended early and never what it ended on.

The lanes are `Game::leading_victory_lanes`: the maximum of the same race
progress `victory_threat` takes, read through `victory_lane_open` so an arena
can only ever be named for the battle it is actually deciding, and with Score
left out for the reason `odds` leaves it out of the race term — it is the
standing when the clock runs out, not a race. Every lane tied at the front is
named. A seat that crossed on standing and tempo with no race under way keeps
the bare `Mercy Rule`.

`Game::mercy_lanes` is read at the crossing, because the board that produced
it stops changing the moment the game does, and it travels into `Decided` when
a world plays on past the result. The victory *type* is untouched — still
`mercy`, which is what `set_winner`, `play_on_blocks` and the saves key off.
Only the label changes, and it is composed in one place
(`Game::victory_label`, `game::mercy_label`) so the finish screen, the play-on
plate, `/state` (`victory_label`), the league's `matches.csv` and the Elo
tournament log all denote a win the same way.

Lanes are joined with ` + ` and never a comma: the label goes into the
`victory` column of `matches.csv`, whose two readers — `backfill_win_profiles`
and `rating::parse_matches_csv` — cut rows on commas, so one comma in the
notation would shift every later column and take the recorded history with it.

Determinism: the crossing ends games, so `odds.rs` computes its
transcendentals (`pow`, `log`) through `libm` — the same soft-float rule as
world geometry (docs/FLOAT_DETERMINISM.md). The display-only
`elo::win_shares` keeps the platform version.

## Require N Victory Types (setup option)

A winner must hold N distinct victory types (setup ladder 1–6, clamped live
to the number of enabled victory conditions; 1 is the stock game). Above 1,
each achieved type **banks** instead of ending the world — Science by
reaching the exoplanet, Culture by the culture victory, Religion by world
conversion, Diplomacy at 20 Diplomatic Victory points, Domination by taking
every other capital — and Score is banked at the turn limit by the leading
scorer through the shipped tiebreak chain. First seat holding N different
types wins with the completing type; if the clock runs out first, the seat
with the most banked types takes the turn-limit award (victory type
`score`, cap scorer breaking ties). Banked sets live in the save
(`victories_won`) and in `/state`. Mercy stays terminal under Require-N: it
is a judgement about the whole game, not one lane.

## The original throughput hypothesis

`docs/closed/FLEET.md` records that 62% of an audited league history ended as score
truncations at turn 250 — games whose tails looked like compute spent on a
settled outcome. The engine already carries a calibrated live win-probability
model (`src/odds.rs`, Brier- and log-loss-checked at three phases of
completed games) whose only consumer was the spectator ribbon. The proposal:
from some mid-game turn, if the odds leader's `now` share crosses a high
threshold, declare it the winner and stop — projected at the time as a
30–40% cut in mean game cost.

That projection was never measured. This document is the measurement.

## The instrument

`civvis odds-audit` plays every game to its real end and records, per
threshold, the first world turn the odds leader crossed it. Nothing about
any outcome changes, so winner agreement and turns saved are exact rather
than estimated:

```
civvis odds-audit --games 60 --players 6 --jobs 10                # stock budget
civvis odds-audit --games 60 --players 6 --turns 250 --jobs 10    # capped
```

Options: `--adjudicate-start` (first sampled turn, default 100), `--every`
(sampling stride, default 5), `--thresholds` (default 0.90,0.95,0.98,0.995).
Seats get a flat 1500 prior — stock fleets, no roster — so only the board
terms and the clock separate the table. `score` endings are the turn cap's
truncation award; every other ending is a game the rules finished.

## Results — 60 seeds each, 6 players, Online speed, 2026-08-07

**Stock turn budget** (the league/exhibition regime; every game ended
naturally — 37 science, 12 diplomatic, 9 religious, 2 culture):

| threshold | crossed | agreement | mean turns saved | share of total compute |
|---|---|---|---|---|
| 0.90 | 31/60 | 93.5% (29/31) | 20.2 | **2.7%** |
| 0.95 | 23/60 | 91.3% (21/23) | 9.6 | 1.0% |
| 0.98 | 11/60 | 100% (11/11) | 4.6 | **0.2%** |
| 0.995 | 6/60 | 100% (6/6) | 2.7 | 0.1% |

**250-turn cap** (the truncation regime the hypothesis lived on; 59/60
games truncated on score):

| threshold | crossed | agreement | mean turns saved | share of total compute |
|---|---|---|---|---|
| 0.90 | 5/60 | 100% (5/5) | 21.2 | **0.7%** |
| 0.95 | 4/60 | 100% (4/4) | 20.2 | 0.5% |
| 0.98 | 4/60 | 100% (4/4) | 9.0 | 0.2% |
| 0.995 | 3/60 | 100% (3/3) | 0.3 | 0.0% |

## Why the hypothesis dies

Two independent mechanisms, one per regime:

- **Under the stock budget there is no tail to cut.** The model's clock
  multiplier deliberately discounts mid-game leads ("calibration must stay
  soft enough for ordinary late upsets"), so high confidence arrives only a
  handful of turns before the rules end the game anyway. And the thresholds
  safe enough to act on (100% agreement starts at 0.98) are exactly the ones
  that arrive latest.
- **Under a cap, truncated games are undecided games.** A board that reaches
  turn 250 without a natural victory is precisely a board where no seat holds
  a commanding position — the odds leader crossed 0.90 in only 5 of 60 capped
  games. The 62% truncation share measured in FLEET.md was never 62% of games
  with a settled outcome; it was 62% of games the *model itself* declines to
  call. The two regimes between them exhaust the design space: where games
  decide, they end; where they don't end, they aren't decided.

The league already took the real fix in this space: raising `max_turns` to
the stock budget so games end on victories rather than truncations
(`LeagueCfg` comment, `docs/EVAL.md`).

## Do not reopen unless

- a *deliberately aggressive* adjudication calibration is proposed — one
  that trades documented winner-agreement for savings. That is a different
  experiment: it changes recorded outcomes, so it needs the paired-map
  machinery and a pre-registered agreement floor (≥99% on ≥300 games at the
  deployment profile), and this instrument is how it would be screened; or
- the fleet returns to short-cap profiles for a bulk workload, in which case
  rerun both tables first — the result above says the savings still will not
  appear, but the instrument is one command.

Anything else re-proposing "stop decided games early via `odds.rs`" without
new mechanism should cite this document and stop.


## Arenas, 2026-08-08: a new signal, not a reopening

The conclusion above stands for worlds. This section is the case the closing
clause anticipated — a new mechanism, measured on the profile it would deploy
to — and it concerns only the Tactics arena, where the model previously had
no signal at all.

Three blindnesses, found by instrument (0/24 crossings at every threshold in
battles that ended twelve units against one):

- `still_playing` asked the world's question — a city or a Settler — which an
  arena seat never holds, so every no-city seat read as already out, the
  living list came back empty, and the whole table sat at zero for the length
  of the battle.
- The domination race counted capitals. An arena holds none, so the model's
  strongest term (up to `RACE_FULL_ELO`) never moved.
- The material floors are Civ-scaled (`SCORE_FLOOR` 25 against arena army
  weights of five to sixty), crushing what little the lead terms saw.

The fix speaks the arena's own rules: `still_playing` mirrors the arena's
elimination rule (units or a city), and the domination race meter is the share
of surviving strength-weighted army — the same quantity the arena's Score
victory ranks at the clock, reading 50 exactly when the armies are even. The
floors are untouched; the race meter carries the crossing.

Measured on 24 seeds per regime, 2 players, 20x20, adjudicating from turn 40
every 10 turns:

**No cities (attrition; median battle 613 of a 656-turn clock):**

| threshold | crossed | agreement | mean turns saved | share of compute |
| --- | --- | --- | --- | --- |
| 0.90 | 24/24 | 100% (24/24) | 157.0 | 35.3% |
| 0.95 | 22/24 | 100% (22/22) | 139.4 | 28.7% |
| 0.98 | 22/24 | 100% (22/22) | 114.8 | 23.6% |
| 0.995 | 22/24 | 100% (22/22) | 84.4 | 17.4% |

**No cities, random start era:**

| threshold | crossed | agreement | mean turns saved | share of compute |
| --- | --- | --- | --- | --- |
| 0.90 | 23/24 | 100% (23/23) | 222.7 | 38.5% |
| 0.995 | 20/24 | 100% (20/20) | 145.7 | 21.9% |

**One city per side (capture):** 1/24 crossings, 100% on the crossing, 1.4%
saved — benign, not a failure. Those battles end at turns 13-57, decided
suddenly by the city falling, so they do not outlive their result and there
is nothing for adjudication to save. The regimes that bled compute are the
ones rescued.

Why the arena escapes the world's verdict: damage is permanent and nothing on
the field rebuilds an army faster than it is destroyed, so a material lead is
close to irreversible in exactly the way a mid-game Civ lead is not — the
"ordinary late upsets" the world's calibration must stay soft for do not
exist here. The world's own tables are unaffected: every change is behind
`Game::is_arena()` except `any_victory_enabled`, which now reads the
*effective* victory conditions — identical for any world, and on an arena the
difference is precisely that its two lanes are open whatever the lobby left
ticked.

For rated play the tournament still runs `mercy_rule: None`; these tables are
the pre-registered evidence for turning it on there, not the decision. The
lobby defaults to None in worlds, arenas, and every historical scenario;
selecting a threshold applies the same rule to any of them.
