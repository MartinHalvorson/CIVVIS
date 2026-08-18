# Terminal score ordered four screened arms correctly, on four data points

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

A decision-grade screen on the deployment profile is **200 pairs / 400 games**,
about two hours of this box. The registry holds over two hundred evaluator
arms. At that rate the unscreened backlog is not a queue, it is a wall.

`ai_eval` already reports a second statistic beside the win gate: the paired
terminal-score direction, with an exact sign test. It resolves on **nearly
every map** where wins resolve on about a third — 200 of 200 against 71 of 200
on the run below. It is explicitly *not* a promotion input, and
`docs/EVAL.md` has burned the project before on reading score gains as strength.

The question is narrower than "is score a good endpoint", which is settled and
the answer is no. It is: **does the cheap statistic, read at the cheap map
count, predict which arms are worth buying a 200-map win screen for?**

## How it was measured

Four arms screened to a win-based decision this session, with the terminal-score
direction each reported at its first (cheap) run and at 200 maps.

## What it measured

| arm | terminal-score direction, first run | 200-map win verdict |
|---|---|---|
| `advanced_open_water_navy` | **30–10 at 40 maps, p=0.0022** | **PASS, +51 Elo** |
| `advanced_engine_faith_price` | 9–15 at 24 maps, p=0.3075 | null, +5 |
| `advanced_builder_survey` | 90–110 at 200 maps, p=0.1790 | null, −2 |
| `advanced_legal_tactical_candidates` | 0–0, all 24 maps neutral | identical by construction |

The one arm that eventually passed is the one whose cheap secondary was
decisive at the cheap map count, by two orders of magnitude in p. The three
that did not pass were, respectively, wrong-signed, wrong-signed, and exactly
neutral.

⚠ **And the counter-example is in the same table.** At 200 maps the faith arm's
terminal score reached **116–84, p=0.0281** — nominally significant — while its
wins sat at +5 with an interval of −30..+41. Terminal score over-calls when you
give it enough maps, exactly as this repository has always said. It is a
triage prior for *where to spend compute*, and it is not evidence about
strength at any map count.

## What was decided

**Recorded, not encoded.** Four points is an ordering, not a validated filter,
and a filter built on four points would quietly stop arms being screened —
which is the failure this session has spent its whole length undoing. No gate,
no flag, no tooling change.

The working practice it supports, for whoever screens next:

1. Run the arm at **40 pairs**. The win verdict there is nearly always
   `INCONCLUSIVE` and the `resolution:` line will say why — at these break
   rates 40 maps cannot promote anything under about +97 Elo-equivalent.
2. Read the **terminal-score direction** on that same run. It costs nothing
   extra; it is already printed.
3. Buy the 200-pair screen for the arms whose cheap direction is decisive.
4. **Record the pair either way**, so this table grows past four rows and the
   heuristic either earns its keep or is retired on evidence.

The prediction this makes, so it can be wrong: an arm whose 40-map
terminal-score sign test is above about p=0.1 will not pass a 200-map win
screen. Four rows say so. Four rows are not many.
