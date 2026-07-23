# Competitive Civ VI baseline

CIVVIS treats "tournament rules" as two distinct layers:

1. **Official Gathering Storm gameplay.** Every ordinary game mechanic remains
   in scope, including pre-game team rules. This is the deterministic engine
   baseline tracked in [MECHANICS.md](MECHANICS.md) and audited in
   [FIDELITY.md](FIDELITY.md).
2. **Community tournament packages.** Current competitive events commonly use
   Better Balanced Game (BBG), Better Balanced Starts (BBS), Multiplayer Helper
   (MPH), and a spectator/map package. These are versioned mods and lobby tools,
   not one permanent Firaxis ruleset. CPL describes them as community-maintained
   tools used to create a level playing field, while its World Cup uses 4v4 teams
   and a rotating map pool.

## Implemented competitive behavior

- Official free-for-all and pre-game team relationships and team victory rules.
- The stock Moderate disaster default, with all five intensity profiles
  available because tournament settings vary by event (for example, CPL has
  published both intensity-4 and player-voted formats).
- All seven Gathering Storm Special Session families: Military, City-State,
  Religious, Nuclear, and Betrayal Emergencies plus disaster and Military Aid
  Requests. Coalition wars, Holy City control, Gold/Send Aid scoring, the
  200-Grievance trigger, tiered Favor, Relics, permanent penalties, and
  Diplomatic Victory Point rewards execute in engine state.
- Deterministic seeded games, stock map sizes/speeds, selectable maps and
  victories, full save/restore, fog-filtered observations, and an omniscient
  spectator view.
- Headless agent tournaments, paired-seat evaluation, and replayable action
  logs.
- Ruleset overlays through `--mods`, with reference validation before play.
- Dated `cpl-ffa-2026-07` and `cpl-teamers-2026-07` policy presets. They use
  Online pacing by default and enforce CPL's no-Gold/no-strategic-resource
  trades, no city trades, no Military Alliance, one independent city-state
  keep token (including stolen-token recaptures), no same-turn civilization
  resurrection, team relic embargo before turn 20, and five-turn Amani
  reassignment cooldown. The teamers preset also turns Barbarians off.
- The persisted `civ6-tournament` game profile is the default for GUI/CLI new
  games and automatically installs `cpl-ffa-2026-07`, Online pacing, Moderate
  disasters, and all victories. Individual map/speed/disaster/victory controls
  remain available after the profile supplies those lobby defaults.

For a stock 4v4 match:

```bash
civvis play --players 8 --teams 0,0,0,0,1,1,1,1 --speed online --spectate
civvis play --players 8 --tournament-preset cpl-ffa-2026-07 --spectate
civvis play --players 8 --teams 0,0,0,0,1,1,1,1 \
  --tournament-preset cpl-teamers-2026-07 --spectate
```

The policy preset deliberately does not claim to activate BBG or reproduce
BBM/BBS terrain normalization. As of this preset's July 2026 cutoff, the
public repositories' latest releases were BBG 7.4.6, BBS V1.38, MPH v1.7.9,
and BSM v1.2.7; BBM is distributed through Workshop item 3179425402. Those
identities are the import targets for the remaining balance, map, lobby, and
spectator work, not implied features of the policy flag.

The first pinned BBG data is available separately as
`mods/bbg-7.4.6-supported`. Its 15 generic rows cover Qin's adjacent Great
Wall Gold/Faith, Sumeria's river-Farm Food, the current adjacency-card chain,
and Scientific Vanguard/Kolkhoz under the current Communism rework. It is
intentionally not enabled by the tournament preset and does not claim the
rest of BBG:

```bash
civvis play --players 8 --tournament-preset cpl-ffa-2026-07 \
  --mods mods/bbg-7.4.6-supported --spectate
```

## Remaining tournament-specific gaps

| Layer | Current boundary |
|---|---|
| BBG balance/content | The generic runtime now executes `ATTACH_MODIFIER`, arbitrary plot/building/city/district yield changes and modifiers, and an initial owner/requirement set. The checked-in 7.4.6 overlay contains 15 verified modifier rows plus policy/government edits; other BBG effect/requirement families and the full tournament civilization roster remain. CIVVIS currently ships 8 leaders. |
| Balanced starts and maps | The stock-style generator has fairness spacing and four map scripts. Exact BBS/BBM start normalization, remap tokens, and the World Cup map rotation (including Highlands, Seven Seas, and Tilted Axis) remain to be ported. |
| Multiplayer Helper | The policy restrictions above execute in engine state. Draft/pick-ban UI, dynamic turn timers, ready checks, pause/remap voting, reconnect administration, concede detection, and tournament result reporting remain client/server work. |
| Turn mode | The authoritative engine is sequential. Simultaneous-turn ordering, dynamic/hybrid turns during war, and network lockstep remain separate protocol work. |
| Event policy | Drafts, civilization bans, disconnect/reload policy, no-quit enforcement, scheduling, and referee decisions belong to a tournament harness, not `Game` state transitions. |

The practical implementation order is therefore: finish the modifier
interpreter and full civilization roster; import a pinned BBG release as data;
port balanced-start/map algorithms; then add simultaneous multiplayer and the
lobby/referee workflow. A tournament preset should pin every mod version rather
than silently tracking latest releases, so old matches remain reproducible.

## Sources

- [Civilization Players League](https://cpl.gg/) — current competitive community
  and its maintained mod stack.
- [Current CPL in-game rules](https://cpl.gg/rules/in-game-rules/) — lobby
  defaults, trade/alliance restrictions, the city-state token, liberation,
  team relic, and Amani-adjacent administration boundary; the separate
  [exploit rules](https://cpl.gg/rules/exploits/) identify the five-turn Amani
  restriction and other prohibited interactions.
- Pinned public releases: [BBG 7.4.6](https://github.com/CivilizationVIBetterBalancedGame/BetterBalancedGame/releases/tag/7.4.6),
  [BBS V1.38](https://github.com/CivilizationVIBetterBalancedGame/BetterBalancedStarts/releases/tag/V1.38),
  [MPH v1.7.9](https://github.com/CivilizationVIBetterBalancedGame/MultiplayerHelper/releases/tag/v1.7.9),
  and [BSM v1.2.7](https://github.com/CivilizationVIBetterBalancedGame/BetterSpectatorMod/releases/tag/v1.2.7).
- [CPL Better Balanced Maps description](https://cpl.gg/mods/bbm/) — current
  competitive start/map normalization target and Workshop identity.
- CPL examples showing event-specific disaster rules: the
  [GS Player's Championship](https://cpl.gg/tournaments/gs-players-championship/)
  used intensity 4, while [Top 10 Exclusive](https://cpl.gg/top-10-exclusive/)
  made intensity a player vote.
- [Civ VI World Cup](https://cpl.gg/civilization-world-cup/) — 4v4 format and map
  rotation.
- [Official team overview](https://www.civilopedia.net/en-US/standard-rules/concepts/teams_1/)
  and [team diplomacy](https://www.civilopedia.net/en-US/gathering-storm/concepts/teams_2/).
- Official team victory pages for
  [science](https://www.civilopedia.net/en-US/standard-rules/concepts/victory_3/),
  [culture](https://www.civilopedia.net/en-US/standard-rules/concepts/victory_4/),
  [domination](https://www.civilopedia.net/en-US/standard-rules/concepts/victory_2/),
  [religion](https://www.civilopedia.net/en-US/gathering-storm/concepts/victory_5/),
  and [score](https://www.civilopedia.net/en-US/gathering-storm/concepts/victory_6/).
- [Official Gathering Storm environmental-effects rules](https://www.civilopedia.net/en-US/gathering-storm/concepts/environmental_effects/).
- [Official World Congress and Aid Request rules](https://www.civilopedia.net/en-US/gathering-storm/concepts/world_congress/)
  and [Send Aid project entry](https://www.civilopedia.net/en-US/gathering-storm/wonders/project_send_aid/).
- [Official Gathering Storm Emergency rules](https://www.civilopedia.net/en-US/gathering-storm/concepts/emergency_alliances_1/)
  and Firaxis's [June 2019 update notes](https://support.civilization.com/hc/en-us/articles/37662399287443-Patch-Notes-June-18-2019),
  which establish the Military Aid Request's 200-Grievance threshold.
