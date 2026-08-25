# CIVVIS

Try CIVVIS yourself at [civvis.ai](https://www.civvis.ai)

The Lv 4 (Prince) is the highest level beat so far, using computer control to bridge the gap between CIVVIS and Firaxis Civ 6. I try to keep the [YouTube](https://www.youtube.com/@civvis) channel somewhat up to date with the latest progress.

## Genetic Algorithm

### Summary

The CIVVIS AI uses a genetic algorithm that tests and assembles a collection of individual Civ 6 heuristics into a player's overall strategy. An individual Civ 6 heuristic might be some rule like "don't move an unescorted settler next to an enemy barbarian" or "if military strength falls below a given level then build more military units". Collecting hundreds (and probably thousands eventually) of these heuristics together forms an overall player strategy.

### Terminology

- Gene - Each heuristic is represented as a gene. A gene is gated by a flag that can be turned "on" or "off".
- Gene pool - The collection of all available genes (both "on" and "off" genes).
- Genome - The set of "on" heuristics for a player, which together form the player's overall strategy.
- Tournament - A Monte Carlo simulation of many probabilistically-generated player genomes competing in CIVVIS Civ 6 games.

### Genetic Algorithm

1) Vibecode Heuristic Genes: I start out vibecoding general Civ 6 heuristics that I think will help improve out strategy. These form our genes to test out.

2) Tournament: > **Note (2026-08-24) — how tournament genomes are drawn.** Every tournament genome starts from our default genome, so the tournament selects for genes that improve on high-level play rather than on some baseline. From that default, each default-"on" gene has a ¼ chance of turning off and each default-"off" gene has a ¼ chance of turning on. A gene that is on then plays its top version 60% of the time and one of its other versions (picked at random among the rest) 40% of the time; a gene with only one version plays that version.

For each free-for-all game, there is one winner. The "on" genes in this players genome are awarded one win. All genes across all players (including duplicates if a gene is present on multiple players) are incremented 1 "game played".

3) Gene Selection: After a tournament concludes, win rates are calculated for every gene (wins / games played). In a 6 player match with equal players, the expected win rate is 1/6 or 16.67%. Genes with win rates above this are deemed beneficial and have a higher likelihood of defaulting "on" in our best genome. The process looks across the last few tournament too to ensure a gene is consistently demonstrating a beneficial performance. Inconsistent results between tournaments is a sign to me to run more games per tournament. Right now I typically run 1667 games per tournament with 6 players/seats each, for a total of 10,002 seats.

4) Best Genome Verification: The best genome is tested then in an ever-running Civ 6 verification game (in the real Civ 6). I'll watch the game and ideate a new set of heuristics to vibecode out, repeating the Genetic Algorithm loop.

## Dev Process

I don't write the lines of code these days but will operate a step higher — suggesting new features to vibecode, asking questions about how code is structured, requesting various refactors or performance optimizations, monitoring verification games, suggesting new heuristics to add as genes, and analyzing CIVVIS tournament results.

I threw some early progress video demos up on [YouTube](https://www.youtube.com/@civvis). At this point, I run both verification games and tournaments autonomously and indefinitely. Half the videos are just screen recordings of successful games with no audio but I try to eventually throw some commentary in the video description.

The simulator at [civvis.ai](www.civvis.ai) needs some work but should be operational.

Quick simulator UI demo:

[![CIVVIS spectate mode — an AI-vs-AI game on a planet world, in real time](https://github.com/MartinHalvorson/CIVVIS/releases/download/media-exhibition/exhibition.gif)](https://github.com/MartinHalvorson/CIVVIS/releases/download/media-exhibition/exhibition.mp4)

<!-- The demo clip lives on the `media-exhibition` release, not in the tree:
     the pair was 20.8 MB — 48% of every checkout — and read only from here.
     Re-shoot by driving the shipped binary over CDP (the retired readme rig
     is in git history, pre-#1285), then update the release assets. -->
