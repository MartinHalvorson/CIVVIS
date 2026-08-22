# Pre-registration — is the champion's deployment deficit the scale, or the victory set?

Written 2026-07-31 **before this run produced any output**. Agent
`claude-evolver`.

## Why

Two arms measured against stock `advanced` on the promotion matrix's exact
deployment profile came back below parity:

- the embedded gen-14 champion, **42.5%** over 40 maps (seed 61,000,000);
- the live league's six-player leader `g28-28`, **40.0%** over 10 maps
  (seed 63,000,000, `INSUFFICIENT`).

Both were selected in worlds where **religious victory is enabled** — the
champion by `civvis evolve` with the default victory set, `g28-28` by 1011
six-player league games. The matrix deployment profile runs
`--victories science,culture,domination`. `ai_eval` records stock `advanced`
taking **86 of 91 wins by religion** when all six are enabled.

So "the champion does not transfer to deployment" has two candidate causes
that no run so far separates: the **map/player scale**, or the **victory set**
in which it was bred.

This is the sixth check in `civvis-measurement-discipline` — *ask what else
differs between the two arms* — applied to my own result.

## The run, fixed now

Identical to the 42.5% run in every argument except the victory set, and on
the **same seed prefix**, so both readings sit on the same forty maps:

```sh
ai_eval advanced_evolved advanced --pairs 40 --jobs 4 --seed 61000000 \
  --players 6 --width 74 --height 46 --city-states 9 --turns 250 \
  --speed online --map continents --shape planet --poles poles \
  --randomize-civs \
  --victories science,culture,religious,diplomatic,domination,score
```

## The rule, fixed now

- If the champion's paired score rises **above parity** with the full victory
  set on the same maps, the deficit is attributed to the **victory set**, and
  the conclusion is that the promotion gate judges bred genomes in a world
  their selection never saw — a finding about the gate, not about the genome.
- If it stays at or below the 42.5% reading, the deficit is attributed to the
  **scale**, and breeding at the deployment profile is the indicated work.
- If it lands between, the split is recorded as unresolved at forty maps and
  neither attribution is claimed.

Forty maps is a screen. Nothing promotes on this, and no genome, gene, seed or
sample size will be chosen after reading it.
