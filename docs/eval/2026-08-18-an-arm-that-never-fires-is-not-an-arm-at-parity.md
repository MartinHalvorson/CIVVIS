# An arm that never fires is not an arm at parity

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

The 2026-08-18 triage sweep ran `advanced_sea_answers` at 40 pairs and it came
back **0 favoured, 40 neutral, 0 against — on both the win column and the
terminal-score column.** Read as an ordinary result, that is parity: the
treatment is worth nothing.

Read correctly, it is not a result at all. Terminal score is continuous and
breaks on nearly every map — two agents that play even slightly differently
separate on it *somewhere*. Forty maps neutral on both columns does not mean
the arms played close games. It means they played **the same games**, and the
treatment never changed an outcome on this profile.

Those two readings ask for opposite next steps. A null asks for a longer
screen. This asks for a mechanism check. Buying the first for the second is how
a 200-pair screen — about two hours — gets spent proving nothing twice.

## How it was measured

Wins break on roughly a third of maps by construction on this profile (71 of
200 on the navy screen, 63 of 200 on the builder screen), so an all-neutral
**win** column is ordinary and says little on its own. Terminal score resolved
on 200 of 200 and 163 of 200 in those same runs. The conjunction is what
carries the claim: both columns neutral on every map, and no draw-mixed map
either.

`ai_eval` now checks exactly that and says so.

## What it measured

Against the arm that prompted it, 8 pairs, seed 11400000:

```
paired outcomes: advanced_sea_answers sweeps 0, neutral splits/draws 8, advanced sweeps 0
terminal-score direction: advanced_sea_answers-favored 0, neutral 8, advanced-favored 0; p=1.0000
⚠ nothing differed: all 8 maps were neutral on wins AND on terminal score, so
  advanced_sea_answers and advanced played the same games. The verdict above is not
  evidence about the treatment — it did not fire on this profile.
```

And against an arm that genuinely differs, same seed and shape
(`advanced_without_open_water_navy`): 7 neutral maps, 1 sweep, terminal score
1–7 — **no line printed**. The check is not "the run was quiet"; it is "the
runs were identical".

## What was decided

**Shipped as a note, not a verdict.** The gate still reports what the evidence
supports; this reports what the evidence *is*. Conflating those is how the
maximum-variance interval came to override the anytime evidence in the first
place (#1973), and the same restraint applies here.

This is the third instrument change this session with the same shape, and the
pattern is worth naming: **the evaluator knew things it did not say.** It knew
the interval it was charging was wider than the data warranted (#1973). It knew
the map count could not resolve the effect being tested (#1993). It knew the
two arms had played identical games. In each case a reader with the raw numbers
could have worked it out, and in each case nobody did, for months.

The open item this leaves is `advanced_sea_answers` itself: a registry arm whose
treatment does not fire on the deployment profile. That is a mechanism question
and is not answered here.
