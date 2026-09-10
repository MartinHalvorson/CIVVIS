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
