# Cheap triage over six arms found one worth buying, one harmful, one inert and one degenerate

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`2026-08-18-terminal-score-ordered-four-screened-arms-correctly-on-four-.md`
proposed a working practice on four data points: run an unscreened arm at **40
pairs**, read the terminal-score sign test that comes free with it, and buy the
**200-pair** win screen only where that direction is decisive. It predicted that
an arm above about **p=0.1** will not pass at 200 maps.

This is the first time it was used to spend compute rather than to explain a
result after the fact.

## How it was measured

Six unscreened `advanced_*` arms at 40 pairs each, 6p 74x46, 6 city-states,
Online, 150 turns, seeds 11000000 upward in steps of 100000, `--jobs 8`. About
an hour of this box in total, against roughly twelve hours to screen all six at
200 pairs.

## What it measured

| arm | terminal-score direction | p | triage call |
|---|---|---:|---|
| **`advanced_maintenance_deck`** | **27–13** | **0.0385** | **buy the screen** |
| `advanced_unit_efficiency` | 22–14 | 0.2430 | skip |
| `advanced_maritime_splice` | 19–21 | 0.8746 | skip |
| `advanced_every_lane` | **0–40** | **0.0000** | **decisively against** |
| `advanced_sea_answers` | 0–0, all 40 neutral | 1.0000 | inert |
| `advanced_recon_fleet` | — | — | **degenerate** |

Three of these are worth more than the triage call itself.

**`advanced_every_lane` loses every map it resolves.** Zero of forty favoured,
forty against, p=0.0000. Whatever it does, it does consistently and it is
consistently worse on the score proxy. It is not a candidate for promotion; it
is a candidate for the withhold ledger.

**`advanced_sea_answers` changes nothing at all.** Forty maps, forty neutral —
not "close to parity" but *no difference on any map*. On this profile the
treatment either never fires or never changes an outcome. That is a different
finding from a null and wants a mechanism check, not a longer screen.

**`advanced_recon_fleet` is not an arm.** `ai_eval` refused it at launch:
*"advanced_recon_fleet and advanced both play as advanced; this run measures
advanced against itself and says nothing about either name."* The
reconnaissance quartet was promoted into `AdvancedAi::new()` and the enabling
arm was left behind. A registry check added alongside this round shows it is
one of **ten** such names, all resolving to `advanced` — enabling arms whose
treatment was promoted, and withholding arms whose treatment was deleted.

## What was decided

`advanced_maintenance_deck` bought its 200-pair matrix screen (seed 12000000);
the other five did not. That is the practice working as intended, and it is
also the first opportunity for it to be wrong — if `maintenance_deck` comes
back null, the prediction has its first miss and that belongs in the record
just as loudly.

⚠ The triage is a spending rule and nothing more. `advanced_every_lane` at
p=0.0000 on terminal score is **not** evidence that it loses games, only that
it is the arm most worth a win-based screen if anyone wants to retire it on
evidence. This log's standing lesson is that score and wins part company, and
one of the six rows above (the faith arm, in an earlier round) is exactly that.
