# The Faith price was wrong at four speeds in five and it changes nothing

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`advanced_engine_faith_price` (#1978) corrects a real defect:
`AdvancedAi::religious_spending_with_reserve` priced a religious unit as
`spec.cost * 2.0`, which is the **Standard-speed** rate, while `Game::item_cost`
scales every price by `game_speed`. Online is 50% — the speed the deployment
evaluator and the live bridge both play — so the controller demanded **twice**
the Faith the engine would take, with the 80-Faith reserve applied on top of a
doubled price. Marathon is 300% and it underquoted by a third. It ignored every
discount that moves the number.

It shipped off by default with a 24-map screen reading **+44 Elo-equivalent,
INCONCLUSIVE**. That was a positive sign nobody could act on.

## How it was measured

`ai_eval advanced_engine_faith_price advanced`, **200 pairs / 400 games** at 6p
74x46, 6 city-states, Online, 150 turns, seed 8900000.

## What it measured

**50.7%, Elo-equivalent +5, betting CI 45.7%..55.8%, 28 sweeps to 25 with 147
neutral, INCONCLUSIVE.** The gate resolves about +46 at this length and break
rate, so an interval of −30..+41 centred on +5 is a null, not a short look.

The earlier **+44 at 24 maps was noise**, and the run that produced it could not
have promoted anything under roughly +200 at that length. It should never have
been read as a positive signal, and the `resolution:` line now printed beside
every verdict exists so the next one is not.

The direct probe had already said why the correction cannot matter much: the
controller is **never short of Faith**. It banks a mean 641 per seat to the end
of a 150-turn game, so a 2x overcharge on a missionary is a constraint that
does not bind.

## What was decided

**Withheld.** The literal it replaces is unambiguously wrong and the replacement
is unambiguously right, and it is worth exactly nothing in wins on the profile
that matters. The arm stays for the record.

⚠ Recorded so it is not rediscovered as a promising lead: **a price being wrong
is not evidence that fixing it is valuable.** The binding constraint on
religious spending is not the price, it is that there is nothing the seat wants
to buy — which is the open question the probe left behind and this round does
not answer.
