# CIVVIS

Try CIVVIS yourself at [civvis.ai](https://www.civvis.ai)

The Lv 4 (Prince) is the highest level beat so far, using computer control to bridge the gap between CIVVIS and Firaxis Civ 6. I try to keep the [YouTube](https://www.youtube.com/@civvis) channel somewhat up to date with the latest progress.

## Genetic Algorithm

The CIVVIS AI uses a genetic algorithm that tests and assembles a collection of heuristics into a player's strategy.

- Gene - Each heuristic is represented as a gene. A gene is gated by a flag that can be turned "on" or "off".
- Gene pool - The collection of all available genes (both "on" and "off" genes).
- Genome - The set of "on" heuristics for a player, together forming the player's overall strategy.
- Tournament - A Monte Carlo simulation of many probabilistically generated player genomes competing in CIVVIS Civ 6 games.

For each free-for-all game, there is one winner. The "on" genes in this players genome are awarded one win. After a tournament concludes, win rates are calculated for every gene.

Genes with positive win rates are deemed beneficial and have a higher likelihood of defaulting "on" in our best genome. 

## Dev Process

I don't write the lines of code these days but will operate a step higher — suggesting new features to vibecode, asking questions about how code is structured, monitoring verification games, suggesting new heuristics to add as genes, and analyzing CIVVIS tournament results.

I threw some early progress video demos up on [YouTube](https://www.youtube.com/@civvis)

Quick simulator UI demo:

[![CIVVIS spectate mode — an AI-vs-AI game on a planet world, in real time](https://github.com/MartinHalvorson/CIVVIS/releases/download/media-exhibition/exhibition.gif)](https://github.com/MartinHalvorson/CIVVIS/releases/download/media-exhibition/exhibition.mp4)

<!-- The demo clip lives on the `media-exhibition` release, not in the tree:
     the pair was 20.8 MB — 48% of every checkout — and read only from here.
     Re-shoot by driving the shipped binary over CDP (the retired readme rig
     is in git history, pre-#1285), then update the release assets. -->
