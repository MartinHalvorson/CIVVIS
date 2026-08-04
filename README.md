# CIVVIS

Some early demo videos up on [@civvis YouTube channel](https://www.youtube.com/@civvis)

Mostly vibecoding this out. Still a bit buggy, apologies for any slop. Continues to be a work in progress.

Quick demo:

[![Spectate mode: a whole AI-vs-AI game on a Planet world of canal-ringed islands — six civilizations settle a globe of hexagons, the camera turns the planet and drops onto the Grand Canals II shelves and channels, Babylon puts the first satellite up on turn 189 and lands on the Moon and Mars, and three expeditions race for another star until Babylon's arrives on turn 282 for the science victory](docs/exhibition.gif)](docs/exhibition.mp4)

## Unified AI timing attacks

`advanced_timing_attack` is the default-off evaluation arm for a coordinated
midgame power-spike war. It appoints one target city and one breakthrough, then
shares that exact plan across research, military production, upgrade Gold,
staging, declaration, and the first-city capture. The spectator dossier and
`ai_eval` expose its phase, package readiness, timing, captures, and aborts.

Its frozen 60-pair live-profile screen proved that the lifecycle works but
rejected the broad policy: it exposed 97.7% of treatment seats and scored only
20.8% paired wins. Production `advanced` therefore remains unchanged. The
follow-up `advanced_timing_attack_selective` arm is preregistered to reuse the
same unified executor only when ordinary strategy already chose Conquest and
three of four assault bodies already exist; it gets one appointment and must
stage all four bodies at 1.25 local strength before declaring.

See [docs/WAR_TIMING.md](docs/WAR_TIMING.md) for the frozen mechanism and
promotion gates. A small smoke comparison can be run with:

```sh
cargo run --release --bin ai_eval -- advanced_timing_attack advanced \
  --players 4 --pairs 2 --turns 160 --seed 10100000
```
