# The triage heuristic has its first miss, and it is the half that matters

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`2026-08-18-terminal-score-ordered-four-screened-arms-correctly-on-four-.md`
proposed a spending rule on four data points and stated the prediction it made
so it could be shown wrong:

> an arm whose 40-map terminal-score sign test is above about p=0.1 will not
> pass a 200-map win screen

`2026-08-18-cheap-triage-over-six-arms-...` then used it for real, and committed
in advance: *"if `maintenance_deck` comes back null, the prediction has its
first miss and that belongs in the record just as loudly."*

It came back null. This is that record.

## What it measured

`advanced_maintenance_deck` was the one arm of six the triage said to buy, on a
40-pair terminal-score direction of **27–13, p=0.0385**.

`--matrix`, 200 pairs per profile, seed 12000000:

| profile | score | Elo-equivalent | interval | terminal-score at 200 |
|---|---:|---:|---|---:|
| compact-standard | 50.0% | +0 | −17..+23 | 108–91, p=0.2567 |
| deployment-online | **45.0%** | **−35** | **−74..+1** | 98–102, p=0.8321 |

**`multi-profile promotion gate: RETAIN advanced`.** Not merely a null: on the
deployment profile the point estimate is −35 with an interval whose upper bound
is +1, so if anything this arm leans harmful.

**The terminal-score signal reversed completely.** p=0.0385 favouring at 40
maps; p=0.83 and 98–102 at 200. The cheap reading was noise.

## What was decided

The rule is **half wrong, and it is the useful half that survives.**

- **The high-p direction still has no counterexample.** Every arm above p≈0.1 at
  its screening count has come back null: `advanced_engine_faith_price` (0.31),
  `advanced_builder_survey` (0.18), `advanced_fortify_idle_units` (0.47/0.53),
  `advanced_settler_founds_when_stalled` (1.00), and the three the sweep
  skipped. As a rule for **not** spending two hours, it is intact.
- **The low-p direction is not predictive and should never have been read as
  though it were.** One arm in six clearing p<0.05 on a 40-map sign test is
  exactly what multiple testing produces from six arms and a null: the expected
  count of false positives at α=0.05 over six independent tests is 0.3, and we
  drew one. The heuristic did not identify a winner; it identified the arm that
  happened to be luckiest, and then bought it two hours of compute.

So the practice is corrected rather than retired: **use the cheap direction to
skip, never to select.** An arm below p=0.1 at 40 maps has earned no priority
over any other unscreened arm — it has merely failed to disqualify itself.

⚠ And the sample is still small in the direction that survives. Five nulls
above p=0.1 is not proof that a real +50 arm cannot read p=0.4 at forty maps;
it is the absence of a counterexample so far. The honest statement of the rule
is that it is a way of ordering a queue nobody can afford to run in full, and
that it will occasionally skip something real.

## The separate finding

`advanced_maintenance_deck` at −35 Elo-equivalent (CI −74..+1) on the
deployment profile is the strongest negative signal any arm has produced in
this session's screens. The interval includes +1 so nothing is established, but
it is a better candidate for a retirement screen than for a promotion one.
