# Fortifying the idle half is worth nothing, and so is founding a stalled settler

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`audit`'s per-unit attribution (#1989) ranks what is standing still. After the
open-water navy was promoted (#1997) and removed the Galley, the table reads:

| unit | idle unit-turns | share of major idle | own idle rate |
|---|---:|---:|---:|
| builder | 1066 | 18.2% | 19.6% |
| **archer** | **909** | **15.5%** | **22.0%** |
| scout | 751 | 12.8% | 15.5% |
| battering_ram | 318 | 5.4% | 28.4% |

Archer is military, and `Motion::idle_could_fortify` counts 3,307 major
unit-turns per three games where a land military unit stood still **without
fortifying** — giving up +3 combat strength per turn, capped at +6, about 30%
of its base. The audit's own comment called that column the defect and the rest
description.

`advanced_fortify_idle_units` does exactly what the column asks. It had never
been screened. Neither had `advanced_settler_founds_when_stalled`, which
answers the audit's other standing symptom — a settler unmoved 25+ turns with
574 legal sites reachable.

## How it was measured

Both at **200 pairs**, because #1993 measured that 40 cannot promote anything
under about +97 Elo-equivalent.

- `advanced_fortify_idle_units`: `--matrix`, 200 pairs per profile, seed 9300000.
- `advanced_settler_founds_when_stalled`: 200 pairs, deployment profile, seed 9700000.

## What it measured

**`advanced_fortify_idle_units` — RETAIN advanced, 1/2 profiles cleared.**

| profile | score | Elo-equivalent | interval | resolution | terminal-score p |
|---|---:|---:|---|---:|---:|
| deployment-online | 49.8% | −2 | −35..+29 | +37 | 0.5310 |
| compact-standard | 50.0% | +0 | −14..+16 | +35 | 0.4672 |

Both intervals are tight against their resolutions, so this is a decision and
not a short look. Collecting the free defensive bonus on 3,307 unit-turns per
three games wins nothing.

**`advanced_settler_founds_when_stalled` — null, and very nearly inert.**
50.0%, +0 Elo-equivalent (CI −12..+15), terminal-score direction **20–20,
p=1.0000**. Only **4 of 200 maps broke** and terminal score moved on 40 of 200:
196 maps came out identical. The treatment fires so rarely that there is
almost nothing to price.

## What was decided

**Both withheld.** More usefully, one claim in the tree is corrected rather
than merely left standing: `Motion::idle_could_fortify`'s doc comment said
*"Only this half is a defect; reporting the total invites work on the other
one."* That was an argument from the mechanic and never a measurement, and the
measurement disagrees. The comment now records the screen and says to read the
column as description.

That matters because the audit is the instrument this session has been steering
by. A column labelled "the defect" sends the next agent at a null.

## The triage prediction, now six for six

`2026-08-18-terminal-score-ordered-four-screened-arms-correctly-on-four-.md`
predicted that an arm whose terminal-score sign test sits above about p=0.1
will not pass a 200-map win screen. Two more rows, both consistent:

| arm | terminal-score p | win verdict |
|---|---:|---|
| `advanced_fortify_idle_units` | 0.4672 / 0.5310 | RETAIN |
| `advanced_settler_founds_when_stalled` | 1.0000 | null |

Six rows, one pass, five nulls, no contradiction yet. Still an ordering rather
than a validated filter, and still not encoded anywhere.
