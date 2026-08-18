# A flag that did nothing cost a whole line of investigation

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

#2007 found that the deployment evaluator profile never produces a diplomatic
or culture victory, while the live ladder loses 41 games to diplomatic and 24
to culture. The obvious follow-up: **is a profile that produces them even
constructible?** Seat every chair with an agent that targets the lane and see.

## What it measured, and what that turned out to mean

Four games with `--ais advanced_target_diplomatic` in all six chairs:

```
Winner: China (player 3) by science on turn 239
Winner: Greece (player 2) by score on turn 250
Winner: Aztec (player 5) by science on turn 224
Winner: China (player 3) by score on turn 250
```

No diplomatic victory. Then three games each for culture and domination — and
they came back **identical to the diplomatic runs and to each other**, winner
for winner and turn for turn.

That is not a finding about victory lanes. Identical output across three
different instructions is a tell, and the check took one command:

| invocation | result |
|---|---|
| no `--ais` | China by science, t239 |
| `--ais not_a_real_agent,...` | China by science, t239 |
| `--ais basic,basic,...` | China by science, t239 |

**`civvis simulate` never reads `--ais`.** It seats `AdvancedAi::fleet` and
plays. An unknown controller name is not rejected either, because nothing looks
at it. Only `tournament` parses the flag; the usage line advertises it once for
every subcommand, so it reads as global.

⚠ Had the identical-output tell not been there, this would have been written up
as **"CIVVIS cannot complete a diplomatic, culture or domination victory"** — a
dramatic claim about the engine, drawn entirely from a flag that did nothing.

## What was decided

**Shipped: the flag is refused where it cannot be honoured.**

```
$ civvis simulate --players 4 --ais basic,basic,basic,basic
--ais is not honoured by `simulate`: it seats every chair with the default
controller. Only `tournament` reads it. Remove the flag, or use that.
```

`tournament --ais` is unaffected. A test pins the allowlist, because the
allowlist is the whole guard — and its **first version was wrong**: it included
`arena`, which looks like it should seat entrants and in fact runs from the
committed league roster and never reads the flag either. Checked rather than
assumed, on the second try.

This is the same failure mode as the three evaluator fixes this session, in its
worst form. Those tools knew something and did not say it. This one accepted an
instruction, discarded it, and returned a confident answer to a question nobody
asked.

⚠ The original question is still open and is now known to be unanswerable this
way: whether CIVVIS can complete a diplomatic or culture victory needs a
subcommand that actually seats controllers, which today means `tournament`, or
an evaluator arm. It was not re-run here — the point of this round is that the
first attempt produced nothing and looked like it had produced something.
