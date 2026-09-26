# Critically wounded siege recovery investigation

Status: diagnosis only; no behavior change or strength claim yet.

Native evidence: `civvis-20260926T211003Z`, pinned `72b0d8bee` (before
the linked-support recovery fix in #3778). Bombard `7208992` and Siege
Tower `3080215` were linked. Coordinates below are native offset coordinates.

| Turn | Start HP | Start | Native result |
| --- | ---: | --- | --- |
| 157 | 100 | (5,25) | Advanced to (6,26); Isin at (7,25) dealt 55 damage. |
| 158 | 45 | (6,26) | Moved to (5,26); Cuirassier `3997696` dealt 44 damage. |
| 159 | 1 | (5,26) | Moved to (5,25). |
| 160 | 1 | (5,25) | Moved to (6,25); Field Cannon `4325386` killed it. |

The recovery owner's frozen baseline and #3778 candidate both reproduce the
turn-160 `MOVE_TO (6,25)` on the same archived frame-0 board. This is a
negative diagnostic for #3778, not evidence that its earlier 31-HP Trebuchet
regression failed. That earlier case and this second case must stay separate.

The archived turn-160 input has SHA-256
`18234122da576c7a954ed0e1165843c11cad42a79835a931076b80672d0de753`.
Local evidence is under
`civvis-war-evidence-20260926/linked-recovery-replay/followup-211003-t160/`.

Investigate the actual danger field, legal retreat tiles, formation eligibility,
and last-shot alternatives before selecting a repair. If every turn-160 option
is lethal, replacing its final move with a hold does not establish that the
unit could have survived. Earlier turns must then be checked separately.

The local ignored diagnostic is temporary investigative scaffolding, not a
CI regression. No new game comparison has been run and no native victory or
capture is attributable to this task.
