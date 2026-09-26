# Recovery for combat units linked to support

## Native failure and current-code reproduction

King / Gran Colombia / Tiny Pangaea / Online / Domination game
`civvis-20260926T190901Z` lost to Culture on turn 188. At turn 115, frame 0,
trebuchet `3538969` had 31 HP and shared `(39,4)` with siege tower `2752531`.
Both reported `formation_count=2`. The trebuchet moved to `(38,6)`, and the
Egyptian district at `(38,8)` killed it with a 31-damage strike. Its campaign
target was a different city, Râ-Kedet at `(34,8)`.

This was the second early trebuchet lost to a district strike; the first died
on turn 106. The last recorded state on each turn 116–143 contains no siege
unit. A Bombard first appears on turn 144. This is an observed replacement gap,
not proof of its production cause.

Archive SHA-256:
`2751e3a479dcc8e470bf347f4b1cd4721e3a0d741be55093155700816cdad94a`.
The offline replay truncates events at the first turn-115 state; its SHA-256 is
`4f10c93d9e03f540650d78c3cca32f06fe6fcfe62657bcd805aab0cfd8043e64`.
The clean task baseline `0507ae309` reproduces the exact native
`MOVE_TO (38,6)` with the recorded live-policy bundle. Changing only the two
formation counts to one does not isolate recovery: the planner links the pair
again before advancing. That counterfactual is retained as a negative diagnostic,
not evidence that an unlinked unit would have survived.

## Proposed correction

The siege controller already admits valid combat/support formations through
`siege_train::linked_support_carrier`. The battle planner's recovery pass instead
excludes every linked unit. Thus the support link makes a wounded siege unit
eligible to advance but ineligible for the existing recovery policy.

Reuse that validated formation predicate for recovery, and reserve both members
when the carrier is held or withdrawn. Keep civilian/religious escort handling,
health thresholds, and ordinary movement legality intact. Exercise healthy return
and linked movement as well as holding in place.

## Preregistered validation

First require focused tests to fail on the current behavior, then replay the
exact archived frame before and after the correction with frozen executables.
The recorded host movement is the baseline witness; an offline safer order is
not proof of native survival or a new domination win.

Before reading candidate results, freeze the unmodified-policy evaluator and
compare four fixed-profile seeds, **37780000–37780003**, across source versions.
Use the existing evaluator command in each version:

```sh
victory_eval --domination-pair early-conquest-opening --games 4 --start-seed 37780000 --out <fresh-file>
```

Each version completes both existing toggle arms; compare matching arms across
versions. These are King, four-player, 60×38 Pangaea, Online, Gran Colombia games
with all victory conditions and the recorded live-policy bundle. Rivals are
CIVVIS controllers, not Firaxis AI. Complete every registered game without early
stopping. Record wins, loss type, ending turn, and ownership outcomes; do not
equate foreign-city totals with original major-capital captures. Four seeds are
a diagnostic pilot, not a statistical claim about stronger play.

The full Rust suite, relevant recovery/siege tests, and changed-file formatting
and quality checks remain required before shipping. No native runtime or
difficulty setting is changed by this investigation.
