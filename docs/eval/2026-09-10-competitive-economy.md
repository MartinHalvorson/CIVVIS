# Competitive strategy research and economic timing genes

Research accessed 2026-09-10. This is an ongoing strategy audit, not a claim
that two genes reproduce expert play or improve the deployed win rate.

## Sources and ruleset boundaries

- Herson's [early-game overexplained guide](https://www.youtube.com/watch?v=L86-YHkfxbQ)
  and [midgame guide](https://www.youtube.com/watch?v=z2IctrOY0sU) identify their
  ruleset as BBG Expanded. His earlier
  [Magnus/commercial-hub opener](https://www.youtube.com/watch?v=iHzDeFGafKU)
  is indexed with an introductory transcript on
  [Glasp](https://glasp.co/youtube/iHzDeFGafKU). The complete videos/transcripts
  could not be retrieved in this session. They are research leads, not
  evidence that an exact timing or parameter below is Herson's recommendation.
  No current competitive ranking was verified.
- The community-authored [multiplayer commercial-hub opener](https://github.com/kat-rng/Civ6-MP-Wiki/blob/main/CommericalHubOpener.md)
  describes an economy built around capacity buildings, internal food and
  production, and a developed trade destination. It connects growth to district
  population requirements and concentrates Government Plaza/Diplomatic Quarter
  trade bonuses. Its governor descriptions differ from unmodified Gathering
  Storm. Transfer the dependency between growth and productive capacity;
  do not hard-code its governor numbers or assume its author is a top-ranked
  player. The new route gene is our engineering inference from that dependency.
- Machiavelli24's [Feudalism wave](https://forums.civfanatics.com/resources/the-feudalism-wave.25723/)
  describes preparing Builders with Ilkum, withholding completion until
  Serfdom is installed, then releasing the completed units. The six-farm
  inspiration and earlier military/settler production are supporting stages.
  This is a 2016 primary strategy write-up, not verified current tournament
  advice. The implemented gene covers the policy-at-completion dependency;
  prebuilding and queue suspension remain separate work.
- Victoria's [district-discount analysis](https://forums.civfanatics.com/resources/civ-vi-district-discounts.27783/)
  motivates auditing unlock order and placement cost together. This branch
  does not implement a district-discount scheduler. A future implementation
  must inspect the engine's cost lock and placement accounting before delaying
  research; a raw preference for cheap districts cannot encode this strategy.
- ATEX's [turn-99 science report](https://forums.civfanatics.com/threads/fastest-science-victory-so-far-turn-99-sv-at-standard-speed.673622/)
  supplies unusually concrete expert-play evidence: an explicit setup,
  milestones, and attached start/end saves. It uses deliberately favorable
  settings, many optional modes, Hungary levies, conquest and pillaging.
  Its transferable lesson is coordination of research, civic, governor and
  production arrival times. Late science needs sufficient culture and builders,
  not merely more science. The thread also discusses dependence on the future
  technology tree. This session read the report, not the save files, and does
  not independently certify a record. These settings are not a benchmark for
  the standard native screen.

## Implemented hypotheses

`trade-growth-to-district` adds a bounded premium to the existing route
valuation when its origin is at population 3, 6 or 9, has filled its current
specialty slots and can use more food. It estimates turns saved toward the
next citizen within twenty Standard turns. Housing headroom, amenities,
loyalty, treasury distress and the remaining game clock can suppress the
premium. It uses the same observed origin/destination yields as the stock
valuation, including food on international routes. It does not manufacture
Magnus bonuses or bypass route legality, alliance and tourism valuation, or
travel costs. The weight is an unmeasured hypothesis: two score points per
Standard turn saved, capped at 24. Growth multipliers are not forecast; this
is a bounded approximation, not an exact population arrival simulation.

`builder-charge-window` puts Serfdom ahead of ordinary economic preferences
when a front-of-queue Builder is within three Standard turns of completion.
It can replace a protected ordinary economic card, which list reordering
alone cannot accomplish in the current policy desk. It preserves explicitly
protected amenity, culture-defense and competition cards, military cards,
and a Settler production card serving a queued Settler. Existing slot legality
and host policy choices still apply. During the window an already installed
Serfdom remains wanted; afterwards ordinary policy selection can reclaim the
slot. It neither inserts Builder queues nor postpones civic research.

Engine authorities inspected: `Game::city_specialty_district_count`, district
placement capacity in `src/game/city.rs`, `Game::growth_cost`, the food
consumption in city processing, `Game::available_policies`,
`Game::item_remaining_cost_for_city`, `Game::item_prod_mult`, and
`Game::builder_charges`. The last currently grants Serfdom's two charges
explicitly. Public Works is not generalized into this gene: its separate
charge-effect handling needs a fidelity audit before claiming equivalence.

Both genes are registered opt-ins, default off, with independent toggles.
Neither is promoted on the strength of these sources or a small probe.

## Validation (2026-09-14)

The five focused tests pass through actual route valuation and policy selection,
including a Builder completing through normal turns with two extra charges and
Serfdom releasing its slot after the completion window. The full locked CI-profile
suite passes: 3,472 library tests, 49 ignored, plus binary, integration, protocol,
and documentation targets. The 206 Python gene/manifest tests and the gene,
manifest, genome-cost, and firing-evidence checks also pass.

Each gene completed a separate six-game, 36-seat Emperor-major probe at the
standard 6-player, 74x46 Continents, Online/250-turn shape. Seeds are
913344700–705 for `builder-charge-window` and 913344800–805 for
`trade-growth-to-district`. The committed artifacts are
`docs/gene_screens/fires/2026-09-10-competitive-economy-*.json`; raw rows are
retained in the session's local run directory. These are smoke probes, not ledger
sources. Their win-difference resolution is approximately ±45.8 and ±44.2
percentage points respectively, so they do not support a strength claim or a
default change. Both hypotheses remain independent, default-off opt-ins.

Further strategy work includes district discount/cost-lock timing, prebuilding
Builders before the policy window, and coordinated victory arrival times. Those
require their own implementation and evaluation.
