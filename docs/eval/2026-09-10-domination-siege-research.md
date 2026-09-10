# Research the missing siege capability

A Conquest plan can name a walled city before the army can take it. The
mobilization path can then raise the production budget, but production cannot
train a siege design its player has never researched. The existing wartime
modernization goal only upgrades at least two standing units of a kind, only
during an active major war. It cannot request the first siege unit before the
war begins. Generic technology values do not guarantee that prerequisite path.

`domination-siege-research` is an independently screenable opt-in. For a
Conquest plan with a legal enemy objective and observed walls, it chooses the
lowest remaining research cost among missing land siege designs. It does
nothing if a land siege unit is already fielded or queued, or a non-obsolete
siege design is already unlocked. A production or resource shortage is not
mistaken for a missing technology. Civilization replacements and obsolete
technologies are respected. In a live observation profile, only the last
reported wall state is used; unknown walls do not trigger this beeline.

The goal joins the existing prerequisite picker after defensive research,
appointed war and air breakthroughs, and wartime modernization. It precedes
optional Prophet, luxury and bargain detours while the missing capability
holds up the campaign. Existing research is not interrupted. Expansion,
Recovery and peaceful lane plans do not request the goal. A Conquest response
to a rival victory can use it as well as an explicit Domination target.

This is a first-capability hypothesis, not a full military research tree.
Once a design is unlocked, force composition, material supply, production,
upgrades, and whether that unit is strong enough for later defenses remain
separate decisions. The gene stays off unless the normal measured deployment
rule selects it; a mechanism test alone is not promotion evidence.

## Validation and probes

- Full `cargo test --profile ci --locked`: 3,576 passed, 52 ignored, zero
  failures. The three mechanism tests include a real prewar Engineering
  research order and unknown, observed and stale wall reports.
- `cargo clippy --profile ci --locked --lib --message-format=json`: no
  compiler or Clippy diagnostics. Formatting, generated-document checks and
  all 14 append-point tests pass after preserving registry ordinals.
- Matched `victory_eval` probe on clean source
  `5487f7f9458263e89c30340de2835d4fbe245a47`: eight three-player games,
  36×22, Online, 250 turns, seeds 109105000–109105007, explicit Domination
  targets in every seat. One leg uses `--with domination-siege-research`,
  the other `--without domination-siege-research`. Each completed Domination
  in 3/8 games, with identical reported terminal summaries on all eight seeds.
  This probe establishes no outcome improvement; it cannot establish that
  every intermediate decision was identical.
- A separate fixed-size 24-game standard screen, Emperor, two workers, seeds
  109104000–109104023, is running. Its build header records the same clean
  source and the standard 6p 74×46 Continents/Online/250t/9CS contract.
  Results pending; neither this probe nor the mechanism tests promote it.


A second matched eight-seed probe held `domination-lane-hands-over` on in both
arms and changed only siege research, using seeds 109106000–109106007, three
players, 36×22, Online 250. The merged source was `7b2fa2d5c`. Each arm
completed one Domination victory; all eight reported terminal summaries were
identical. This probe also provides no outcome-benefit evidence. Identical
summaries do not prove every intermediate action was identical.

After merging main, the full Rust suite passed 3,582 tests (52 ignored). The
three research regressions also passed after moving them into their dedicated
test module, and the 14 append-point tests passed. Generated gene and manifest
checks remain current.

A read-only `run_game_observed` replay of those same eight handover seeds,
linked against the merged evaluation build, recorded every major's selected
research and known technology set at each turn start. All 5,937 snapshots
matched between arms, as did the eight terminal records. This is stronger
negative evidence than matching final summaries: the option had no observed
research effect in this regime. The snapshot timing does not observe every
individual action, and this result says nothing about untested map profiles.

A separate observer wrapper counted Conquest plans after each seat's turn.
Across the same eight control games, it saw 871 observations with a still-walled
foreign objective; every one already had Engineering. The wrapper reproduced
all eight terminal records. This explains why first-capability research was
not a binding constraint in that native probe. It does not establish that the
siege units were actually built, delivered or strong enough for later walls.
Those are the more useful follow-up questions for this tested regime.

The [campaign census](2026-09-10-domination-siege-census.json) preserves the
configuration, per-seat counts and observer source. Of the 871 walled-objective
observations, 230 had no fielded land siege unit and 245 had one but none within
six hexes. Narrowing to active wars gives 300 observations: 81 had neither a
fielded nor queued land siege unit; 98 had siege units but none within six
hexes. These are repeated post-turn observations, not independent samples or
proof of a production/pathfinding defect. They include the cost of staging,
and the distance test does not establish reachability or firing capability.

## Existing siege-commitment follow-up

An exploratory replay enabling the existing `siege-commitment` gene completed
Domination twice in the eight seeds, versus once in the control. That small
result did not carry into a predeclared, disjoint 40-seed comparison
(109107000–109107039; same source and shape, handover on and siege research off
in both arms). Siege commitment completed Domination in **5/40** worlds versus
**10/40** without it. The paired seeds lost five completions and gained none;
26 terminal records were identical. The two-sided exact test on the five
discordant outcomes is p=0.0625, so this is negative evidence for the proposed
use, not a precise universal effect estimate.

Active-war walled-objective observations were 1,349 with commitment and 1,127
without it. Of these, no fielded or queued land siege unit was present in
216 versus 80 observations; a fielded siege unit existed but none was within
six hexes in 486 versus 402. These different trajectories do not isolate the
cause of the lost completions. A stickier target is not itself a remedy for
an unexecutable siege. No deployment default changed. The census artifact
includes every paired terminal record, protocol and observer source.

The native probes and census use Prince, the source revision’s default
`GameOptions` difficulty. The standard screen separately uses Emperor.
