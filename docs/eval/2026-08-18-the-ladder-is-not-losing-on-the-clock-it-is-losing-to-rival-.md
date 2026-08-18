# The ladder is not losing on the clock, it is losing to rival victories

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`docs/EVAL_STATUS.md` reports the live ladder as **310 configured attempts, 3
configured wins**. That is a statistic that moves about twice a month, so it
cannot say whether the bridge is improving between wins — and it has been
improving sharply: over the attempts that ran the full clock, the daily median
terminal score ran 200–520 through 2026-08-15 and then **716, 977, 946** on the
three days after the actuation-gap work landed. None of that was visible on the
page the project reads.

`rival_best` began being recorded on 2026-08-16, which turns a score into a
distance. So: what does the ladder look like when the ledger is asked for
distance rather than for wins?

## What it measured

The snapshot now carries two more lines, and the second one is the finding.

**Distance.** 133 attempts ran the full clock, median score **449**, best
**1191**. Of those, **17 are graded** against the best rival: rival bar median
**1035**, our lead median **−83**, best **+409**, and **ahead in 6 of 17**.

A median lead of −83 against a bar of 1035 is about 8% short. The headline of
three wins in three hundred attempts does not describe that seat.

**⚠ And most games do not reach the clock at all.**

| lost to a rival's victory before turn 248 | count |
|---|---:|
| diplomatic | 41 |
| culture | 24 |
| religious | 5 |
| technology | 3 |
| conquest | 1 |
| **total** | **74 of 310 configured attempts** |

**Three of them ended while our own score was the highest on the board** — one
at turn 243 with a lead of **+409**.

A seat that is ahead on score and loses to somebody else's victory condition
has a **denial** problem, not a development problem. The win count cannot tell
those apart: both are `won=False`. Nor can the terminal-score median, which is
the statistic the improving trend showed up in — those two numbers together
would have suggested the bridge was getting stronger and merely unlucky.

## What was decided

**Shipped as reporting.** `tools/eval_manifest.py` computes both blocks and CI's
existing `--check` keeps them current, so this cannot go stale the way the
prose numbers it replaces did.

No controller change follows, and one is not obviously needed yet:
`AdvancedAi::new` already carries `deny_while_targeted` and
`stock_denial_lead_time`, both added on this exact observation
(*"five of the twelve runs the seat was LEADING on 2026-08-16/17 ended at
t229–245 on a rival's culture, technology or diplomatic victory"*). What was
missing was any way to see whether they worked. The graded rows here span
2026-08-16 to 08-18 and straddle that change, and the theft continues —
including the +409 game on 08-17 — so on the evidence available they have not
closed it.

That is now a question the page asks every time it is regenerated, rather than
one somebody has to go and re-derive from a 317-row JSON file.

⚠ Seventeen graded rows is not many, and every one of them is recent. The
distance numbers describe the bridge as it is now, which is the point, but they
are not a trend and should not be read as one.
